use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::time::Instant;

use crate::HttpClient;

use super::config::{RunConfig, ScenarioConfig, ScenarioExecutor, ScriptOptions};
use super::error::{Error, Result};
use super::gate::IterationGate;
use super::pacer::ArrivalPacer;
use super::progress::{LiveMetrics, ProgressFn, ProgressUpdate, ScenarioProgress, StageProgress};
use super::schedule::RampingU64Schedule;
use super::shared::SharedStore;
use super::stats::{RunStats, RunSummary};
use super::vu::{EnvVars, StartSignal, VuContext, VuWork};
use tokio::sync::Barrier;
use tokio::time::MissedTickBehavior;

pub fn scenarios_from_options(opts: ScriptOptions, cfg: RunConfig) -> Result<Vec<ScenarioConfig>> {
    let cli_overrides_set = cfg.vus.is_some() || cfg.iterations.is_some() || cfg.duration.is_some();

    // If `options.scenarios` exists, it wins. Otherwise we fall back to top-level options.
    if !opts.scenarios.is_empty() {
        let mut out = Vec::with_capacity(opts.scenarios.len());
        for s in opts.scenarios {
            let exec = s.exec.unwrap_or_else(|| "Default".to_string());
            let executor_name = s.executor.as_deref().unwrap_or("constant-vus");

            // CLI flags have the highest priority. If the script defines a ramping executor but the
            // user explicitly requested a different run shape via CLI (iterations/vus/duration),
            // treat it as a constant VU scenario and ignore ramping-specific fields.
            if cli_overrides_set
                && matches!(
                    executor_name,
                    "ramping-vus" | "ramping-arrival-rate" | "ramping-rps"
                )
            {
                let vus = cfg.vus.or(s.vus).or(opts.vus).unwrap_or(1);
                if vus == 0 {
                    return Err(Error::InvalidVus);
                }

                let iterations = cfg.iterations.or(s.iterations).or(opts.iterations);
                if iterations == Some(0) {
                    return Err(Error::InvalidIterations);
                }

                let duration = cfg.duration.or(s.duration).or(opts.duration);

                out.push(ScenarioConfig {
                    name: s.name,
                    exec,
                    executor: ScenarioExecutor::ConstantVus { vus },
                    iterations,
                    duration,
                });
                continue;
            }

            match executor_name {
                "constant-vus" | "constant" | "per-vu-iterations" => {
                    let vus = cfg.vus.or(s.vus).or(opts.vus).unwrap_or(1);
                    if vus == 0 {
                        return Err(Error::InvalidVus);
                    }

                    let iterations = cfg.iterations.or(s.iterations).or(opts.iterations);
                    if iterations == Some(0) {
                        return Err(Error::InvalidIterations);
                    }

                    let duration = cfg.duration.or(s.duration).or(opts.duration);

                    out.push(ScenarioConfig {
                        name: s.name,
                        exec,
                        executor: ScenarioExecutor::ConstantVus { vus },
                        iterations,
                        duration,
                    });
                }
                "ramping-vus" => {
                    if s.iterations.is_some() || opts.iterations.is_some() {
                        return Err(Error::InvalidIterations);
                    }
                    if s.stages.is_empty() {
                        return Err(Error::InvalidStages);
                    }

                    let start_vus = s.start_vus.unwrap_or(0);
                    let max_stage = s.stages.iter().map(|st| st.target).max().unwrap_or(0);
                    let max_vus = max_stage.max(start_vus);
                    if max_vus == 0 {
                        return Err(Error::InvalidVus);
                    }

                    let total_duration =
                        s.stages.iter().fold(std::time::Duration::ZERO, |acc, st| {
                            acc.saturating_add(st.duration)
                        });
                    if total_duration.is_zero() {
                        return Err(Error::InvalidStages);
                    }

                    out.push(ScenarioConfig {
                        name: s.name,
                        exec,
                        executor: ScenarioExecutor::RampingVus {
                            start_vus,
                            stages: s.stages,
                        },
                        iterations: None,
                        duration: Some(total_duration),
                    });
                }
                "ramping-arrival-rate" | "ramping-rps" => {
                    if s.iterations.is_some() || opts.iterations.is_some() {
                        return Err(Error::InvalidIterations);
                    }
                    if s.stages.is_empty() {
                        return Err(Error::InvalidStages);
                    }

                    let start_rate = s.start_rate.unwrap_or(0);
                    let time_unit = s.time_unit.unwrap_or(std::time::Duration::from_secs(1));
                    if time_unit.is_zero() {
                        return Err(Error::InvalidTimeUnit);
                    }

                    let pre_allocated_vus = s.pre_allocated_vus.unwrap_or(1);
                    if pre_allocated_vus == 0 {
                        return Err(Error::InvalidPreAllocatedVus);
                    }

                    let max_vus = s.max_vus.unwrap_or(pre_allocated_vus);
                    if max_vus < pre_allocated_vus {
                        return Err(Error::InvalidMaxVus);
                    }

                    let total_duration =
                        s.stages.iter().fold(std::time::Duration::ZERO, |acc, st| {
                            acc.saturating_add(st.duration)
                        });
                    if total_duration.is_zero() {
                        return Err(Error::InvalidStages);
                    }

                    out.push(ScenarioConfig {
                        name: s.name,
                        exec,
                        executor: ScenarioExecutor::RampingArrivalRate {
                            start_rate,
                            time_unit,
                            pre_allocated_vus,
                            max_vus,
                            stages: s.stages,
                        },
                        iterations: None,
                        duration: Some(total_duration),
                    });
                }
                _ => return Err(Error::InvalidExecutor),
            }
        }
        return Ok(out);
    }

    let vus = cfg.vus.or(opts.vus).unwrap_or(1);
    if vus == 0 {
        return Err(Error::InvalidVus);
    }

    // Default iterations is 1 unless duration mode is used.
    let iterations = cfg.iterations.or(opts.iterations).or_else(|| {
        if cfg.duration.or(opts.duration).is_some() {
            None
        } else {
            Some(1)
        }
    });
    if iterations == Some(0) {
        return Err(Error::InvalidIterations);
    }

    let duration = cfg.duration.or(opts.duration);

    Ok(vec![ScenarioConfig {
        name: "Default".to_string(),
        exec: "Default".to_string(),
        executor: ScenarioExecutor::ConstantVus { vus },
        iterations,
        duration,
    }])
}

pub fn process_env_snapshot() -> EnvVars {
    let vars: Vec<(Arc<str>, Arc<str>)> = std::env::vars()
        .map(|(k, v)| (Arc::<str>::from(k), Arc::<str>::from(v)))
        .collect();
    Arc::from(vars.into_boxed_slice())
}

pub async fn run_scenarios<F, Fut, E>(
    script: &str,
    script_path: Option<&Path>,
    scenarios: Vec<ScenarioConfig>,
    env: EnvVars,
    shared: Arc<SharedStore>,
    vu: F,
    progress: Option<ProgressFn>,
) -> Result<RunSummary>
where
    F: Fn(VuContext) -> Fut + Clone + Send + Sync + 'static,
    Fut: std::future::Future<Output = std::result::Result<(), E>> + Send + 'static,
    E: std::error::Error + Send + Sync + 'static,
{
    let client = Arc::new(HttpClient::default());
    let stats = Arc::new(RunStats::default());

    let script: Arc<str> = Arc::from(script);
    let script_path = script_path.map(|p| Arc::new(p.to_path_buf()));
    let shared = shared;

    let scenario_max_vus = |s: &ScenarioConfig| -> u64 {
        match &s.executor {
            ScenarioExecutor::ConstantVus { vus } => *vus,
            ScenarioExecutor::RampingVus { start_vus, stages } => {
                let max_stage = stages.iter().map(|st| st.target).max().unwrap_or(0);
                max_stage.max(*start_vus)
            }
            ScenarioExecutor::RampingArrivalRate { max_vus, .. } => *max_vus,
        }
    };

    let total_vus: usize = scenarios
        .iter()
        .map(|s| scenario_max_vus(s).min(usize::MAX as u64) as usize)
        .sum();
    let init_error: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let ready_barrier: Arc<Barrier> = Arc::new(Barrier::new(total_vus.saturating_add(1)));
    let start_signal: Arc<StartSignal> = Arc::new(StartSignal::new());
    let run_started: Arc<OnceLock<Instant>> = Arc::new(OnceLock::new());

    let mut scenario_gates: Vec<Arc<IterationGate>> = Vec::new();
    let mut pacers: Vec<(
        Arc<ArrivalPacer>,
        Arc<RampingU64Schedule>,
        std::time::Duration,
        std::time::Duration,
    )> = Vec::new();

    #[derive(Clone)]
    enum ScenarioProgressInfo {
        ConstantVus {
            vus: u64,
            duration: Option<std::time::Duration>,
        },
        RampingVus {
            schedule: Arc<RampingU64Schedule>,
        },
        RampingArrivalRate {
            schedule: Arc<RampingU64Schedule>,
            time_unit: std::time::Duration,
            pacer: Arc<ArrivalPacer>,
            max_vus: u64,
        },
    }

    #[derive(Clone)]
    struct ProgressScenario {
        name: String,
        exec: String,
        progress: ScenarioProgressInfo,
    }

    let mut progress_scenarios: Vec<ProgressScenario> = Vec::new();

    let mut next_vu_id: u64 = 1;

    let mut handles = Vec::with_capacity(total_vus);
    for scenario in scenarios {
        let scenario_vus_max = scenario_max_vus(&scenario);
        let scenario_name_string = scenario.name.clone();
        let exec_string = scenario.exec.clone();

        let work = match &scenario.executor {
            ScenarioExecutor::ConstantVus { vus } => {
                let gate = Arc::new(IterationGate::new(scenario.iterations, scenario.duration));
                scenario_gates.push(gate.clone());

                if progress.is_some() {
                    progress_scenarios.push(ProgressScenario {
                        name: scenario_name_string.clone(),
                        exec: exec_string.clone(),
                        progress: ScenarioProgressInfo::ConstantVus {
                            vus: *vus,
                            duration: scenario.duration,
                        },
                    });
                }

                VuWork::Constant { gate }
            }
            ScenarioExecutor::RampingVus { start_vus, stages } => {
                let schedule = Arc::new(RampingU64Schedule::new(*start_vus, stages.clone()));

                if progress.is_some() {
                    progress_scenarios.push(ProgressScenario {
                        name: scenario_name_string.clone(),
                        exec: exec_string.clone(),
                        progress: ScenarioProgressInfo::RampingVus {
                            schedule: schedule.clone(),
                        },
                    });
                }

                VuWork::RampingVus { schedule }
            }
            ScenarioExecutor::RampingArrivalRate {
                start_rate,
                time_unit,
                pre_allocated_vus,
                max_vus,
                stages,
            } => {
                let schedule = Arc::new(RampingU64Schedule::new(*start_rate, stages.clone()));
                let pacer = Arc::new(ArrivalPacer::new(*pre_allocated_vus, *max_vus));

                if progress.is_some() {
                    progress_scenarios.push(ProgressScenario {
                        name: scenario_name_string.clone(),
                        exec: exec_string.clone(),
                        progress: ScenarioProgressInfo::RampingArrivalRate {
                            schedule: schedule.clone(),
                            time_unit: *time_unit,
                            pacer: pacer.clone(),
                            max_vus: *max_vus,
                        },
                    });
                }

                pacers.push((
                    pacer.clone(),
                    schedule.clone(),
                    *time_unit,
                    schedule.total_duration(),
                ));
                VuWork::RampingArrivalRate {
                    schedule,
                    time_unit: *time_unit,
                    pacer,
                }
            }
        };

        let scenario_name: Arc<str> = Arc::from(scenario.name);
        let exec: Arc<str> = Arc::from(scenario.exec);

        for scenario_vu in 1..=scenario_vus_max {
            let vu_id = next_vu_id;
            next_vu_id = next_vu_id.saturating_add(1);
            let ctx = VuContext {
                vu_id,
                scenario: scenario_name.clone(),
                scenario_vu,
                script: script.clone(),
                script_path: script_path.clone(),
                exec: exec.clone(),
                client: client.clone(),
                stats: stats.clone(),
                work: work.clone(),
                env: env.clone(),

                shared: shared.clone(),

                run_started: run_started.clone(),

                init_error: init_error.clone(),
                ready_barrier: ready_barrier.clone(),
                start_signal: start_signal.clone(),
            };

            let vu = vu.clone();
            handles.push(tokio::spawn(async move {
                vu(ctx).await.map_err(|err| Error::Vu(err.to_string()))
            }));
        }
    }

    // Block until all VUs have created their Lua state and loaded the script.
    // This keeps initialization out of the measured runtime and avoids per-VU start skew.
    ready_barrier.wait().await;

    let init_err = init_error
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone();

    if let Some(err) = init_err {
        for h in &handles {
            h.abort();
        }

        // Ensure tasks have a chance to observe the abort before we return.
        for h in handles {
            let _ = h.await;
        }

        return Err(Error::Vu(err));
    }

    let started = Instant::now();
    let _ = run_started.set(started);
    for gate in scenario_gates {
        gate.start_at(started);
    }
    start_signal.start();

    let progress_handle = progress.as_ref().map(|progress| {
        let progress = progress.clone();
        let scenarios = progress_scenarios.clone();
        let stats = stats.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
            interval.set_missed_tick_behavior(MissedTickBehavior::Delay);

            let mut tick_id: u64 = 0;
            let mut last_at = Instant::now();
            let mut last_requests_total = stats.requests_total();
            let mut last_bytes_received_total = stats.bytes_received_total();
            let mut last_bytes_sent_total = stats.bytes_sent_total();

            loop {
                interval.tick().await;

                tick_id = tick_id.saturating_add(1);
                let now = Instant::now();
                let dt = now.duration_since(last_at);
                last_at = now;

                let elapsed = started.elapsed();

                let requests_total = stats.requests_total();
                let delta_req = requests_total.saturating_sub(last_requests_total);
                last_requests_total = requests_total;
                let rps_now = (delta_req as f64) / dt.as_secs_f64().max(1e-9);
                stats.record_rps_sample(rps_now);

                let bytes_received_total = stats.bytes_received_total();
                let delta_bytes = bytes_received_total.saturating_sub(last_bytes_received_total);
                last_bytes_received_total = bytes_received_total;
                let bytes_received_per_sec_now =
                    (delta_bytes as f64 / dt.as_secs_f64().max(1e-9)).round() as u64;

                let bytes_sent_total = stats.bytes_sent_total();
                let delta_sent = bytes_sent_total.saturating_sub(last_bytes_sent_total);
                last_bytes_sent_total = bytes_sent_total;
                let bytes_sent_per_sec_now =
                    (delta_sent as f64 / dt.as_secs_f64().max(1e-9)).round() as u64;

                let (lat_p50_ms, lat_p95_ms) = stats.take_latency_window_ms();

                let latency = stats.latency_snapshot_ms();
                let (req_per_sec_avg, req_per_sec_stdev, req_per_sec_max, req_per_sec_stdev_pct) =
                    stats.req_per_sec_summary();
                let checks_failed_total = stats.checks_failed_total();
                let checks_failed = stats.errors_snapshot();

                let iterations_total = stats.iterations_total();
                let failed_requests_total = stats.failed_requests_total();

                let latency_stdev_pct = if latency.mean_ms > 0.0 {
                    (latency.stdev_ms / latency.mean_ms) * 100.0
                } else {
                    0.0
                };

                let metrics = LiveMetrics {
                    rps_now,
                    bytes_received_per_sec_now,
                    bytes_sent_per_sec_now,
                    requests_total,
                    bytes_received_total,
                    bytes_sent_total,
                    failed_requests_total,
                    checks_failed_total,
                    req_per_sec_avg,
                    req_per_sec_stdev,
                    req_per_sec_max,
                    req_per_sec_stdev_pct,
                    latency_mean_ms: latency.mean_ms,
                    latency_stdev_ms: latency.stdev_ms,
                    latency_max_ms: latency.max_ms,
                    latency_p50_ms: latency.p50_ms,
                    latency_p75_ms: latency.p75_ms,
                    latency_p90_ms: latency.p90_ms,
                    latency_p99_ms: latency.p99_ms,
                    latency_stdev_pct,
                    latency_distribution_ms: latency.distribution_ms,
                    checks_failed,
                    latency_p50_ms_now: lat_p50_ms,
                    latency_p95_ms_now: lat_p95_ms,
                    iterations_total,
                };

                for s in &scenarios {
                    let progress_val = match &s.progress {
                        ScenarioProgressInfo::ConstantVus { vus, duration } => {
                            ScenarioProgress::ConstantVus {
                                vus: *vus,
                                duration: *duration,
                            }
                        }
                        ScenarioProgressInfo::RampingVus { schedule } => {
                            let stage =
                                schedule.stage_snapshot_at(elapsed).map(|st| StageProgress {
                                    stage: st.index + 1,
                                    stages: st.count,
                                    stage_elapsed: st.stage_elapsed,
                                    stage_remaining: st.stage_remaining,
                                    start_target: st.start_target,
                                    end_target: st.end_target,
                                    current_target: st.current_target,
                                });
                            ScenarioProgress::RampingVus {
                                total_duration: schedule.total_duration(),
                                stage,
                            }
                        }
                        ScenarioProgressInfo::RampingArrivalRate {
                            schedule,
                            time_unit,
                            pacer,
                            max_vus,
                        } => {
                            let stage =
                                schedule.stage_snapshot_at(elapsed).map(|st| StageProgress {
                                    stage: st.index + 1,
                                    stages: st.count,
                                    stage_elapsed: st.stage_elapsed,
                                    stage_remaining: st.stage_remaining,
                                    start_target: st.start_target,
                                    end_target: st.end_target,
                                    current_target: st.current_target,
                                });

                            ScenarioProgress::RampingArrivalRate {
                                time_unit: *time_unit,
                                total_duration: schedule.total_duration(),
                                stage,
                                active_vus: pacer.active_vus(),
                                max_vus: *max_vus,
                                dropped_iterations_total: pacer.dropped_total(),
                            }
                        }
                    };

                    (progress)(ProgressUpdate {
                        tick: tick_id,
                        elapsed,
                        scenario: s.name.clone(),
                        exec: s.exec.clone(),
                        metrics: metrics.clone(),
                        progress: progress_val,
                    });
                }
            }
        })
    });

    // Start any arrival-rate pacers after we start the VUs (so we don't build up backlog
    // while VUs are still waiting on the start signal).
    for (pacer, schedule, time_unit, total_duration) in pacers {
        let stats = stats.clone();
        handles.push(tokio::spawn(async move {
            let tick = std::time::Duration::from_millis(10);
            let mut interval = tokio::time::interval(tick);
            interval.set_missed_tick_behavior(MissedTickBehavior::Delay);

            let mut carry = 0.0f64;
            let mut last_dropped = 0u64;

            loop {
                interval.tick().await;

                let elapsed = started.elapsed();
                if elapsed >= total_duration {
                    break;
                }

                let rate = schedule.target_at(elapsed) as f64;
                let tick_s = tick.as_secs_f64();
                let unit_s = time_unit.as_secs_f64().max(1e-9);

                carry += rate * (tick_s / unit_s);
                let due = carry.floor() as u64;
                carry -= due as f64;

                pacer.update_due(due);

                let dropped = pacer.dropped_total();
                let delta = dropped.saturating_sub(last_dropped);
                if delta != 0 {
                    stats.record_dropped_iterations(delta);
                    last_dropped = dropped;
                }
            }

            pacer.mark_done();
            Ok(())
        }));
    }

    for h in handles {
        h.await??;
    }

    if let Some(h) = progress_handle {
        h.abort();
        let _ = h.await;
    }

    Ok(stats.summarize(started.elapsed()).await)
}
