use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

mod format;
mod progress;
mod summary;

use format::{format_bytes, format_rate};
use progress::HumanProgress;
use summary::render;

use crate::output::human::format::*;

use super::OutputFormatter;

pub(crate) struct HumanReadableOutput {
    progress: Arc<HumanProgress>,
    max_elapsed_ms: Arc<AtomicU64>,
}

impl HumanReadableOutput {
    pub(crate) fn new() -> Self {
        Self {
            progress: Arc::new(HumanProgress::new()),
            max_elapsed_ms: Arc::new(AtomicU64::new(0)),
        }
    }
}

impl OutputFormatter for HumanReadableOutput {
    fn print_header(&self, script_path: &std::path::Path, scenarios: &[wrkr_core::ScenarioConfig]) {
        println!("script: {}", script_path.display());
        for s in scenarios {
            println!(
                "scenario: {} exec={} iterations={:?} duration={:?}",
                s.metrics_ctx.scenario(),
                s.exec,
                s.iterations,
                s.duration
            );
        }
        if !scenarios.is_empty() {
            println!();
        }
    }

    fn progress(&self) -> Option<wrkr_core::ProgressFn> {
        let progress = self.progress.clone();
        let max_elapsed_ms = self.max_elapsed_ms.clone();
        let prev_iters: Arc<Mutex<HashMap<String, u64>>> = Arc::new(Mutex::new(HashMap::new()));
        let prev_errors: Arc<Mutex<HashMap<String, u64>>> = Arc::new(Mutex::new(HashMap::new()));

        Some(Arc::new(move |u| {
            let elapsed_ms = u.elapsed.as_millis() as u64;
            let mut cur = max_elapsed_ms.load(Ordering::Relaxed);
            while elapsed_ms > cur {
                match max_elapsed_ms.compare_exchange_weak(
                    cur,
                    elapsed_ms,
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => break,
                    Err(observed) => cur = observed,
                }
            }

            let dt = u.interval.as_secs_f64().max(1e-9);
            let iters_total = u.metrics.iterations_total;
            let errors_total = u
                .metrics
                .failed_requests_total
                .saturating_add(u.metrics.checks_failed_total);

            let prev = {
                let mut inner = prev_iters
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                let prev = inner.get(&u.scenario).copied();
                inner.insert(u.scenario.clone(), iters_total);
                prev
            };

            let prev_err = {
                let mut inner = prev_errors
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                let prev = inner.get(&u.scenario).copied();
                inner.insert(u.scenario.clone(), errors_total);
                prev
            };

            let iters_delta = iters_total.saturating_sub(prev.unwrap_or_default());
            let iters_per_sec = (iters_delta as f64) / dt;

            let errors_delta = errors_total.saturating_sub(prev_err.unwrap_or_default());

            let rps = u.metrics.rps_now;
            let throughput_per_sec = u
                .metrics
                .bytes_received_per_sec_now
                .saturating_add(u.metrics.bytes_sent_per_sec_now);

            let rates = format!(
                " iters/s={} rps={} tps={}/s errors={errors_delta}/{errors_total}",
                format_rate(iters_per_sec),
                format_rate(rps),
                format_bytes(throughput_per_sec)
            );

            let (total_duration_opt, message) = match &u.progress {
                wrkr_core::ScenarioProgress::ConstantVus { vus, duration } => (
                    *duration,
                    format!("vus={vus} elapsed={}{}", format_duration(u.elapsed), rates),
                ),
                wrkr_core::ScenarioProgress::RampingVus {
                    total_duration,
                    stage,
                } => {
                    let msg = if let Some(stage) = stage {
                        format!(
                            "stage={}/{} target={} elapsed={} stage_remaining={}{}",
                            stage.stage,
                            stage.stages,
                            stage.current_target,
                            format_duration(u.elapsed),
                            format_duration(stage.stage_remaining),
                            rates
                        )
                    } else {
                        format!("elapsed={}{}", format_duration(u.elapsed), rates)
                    };
                    (Some(*total_duration), msg)
                }
                wrkr_core::ScenarioProgress::RampingArrivalRate {
                    total_duration,
                    stage,
                    active_vus,
                    max_vus,
                    dropped_iterations_total,
                    ..
                } => {
                    let mut msg = format!(
                        "active_vus={active_vus}/{max_vus} dropped={dropped_iterations_total} elapsed={}{}",
                        format_duration(u.elapsed),
                        rates
                    );
                    if let Some(stage) = stage {
                        msg.push_str(&format!(
                            " stage={}/{} target={}",
                            stage.stage, stage.stages, stage.current_target
                        ));
                    }
                    (Some(*total_duration), msg)
                }
            };

            progress.update(&u.scenario, total_duration_opt, u.elapsed, message);
        }))
    }

    fn print_summary(&self, summary: &wrkr_core::RunSummary) -> anyhow::Result<()> {
        self.progress.finish();
        let elapsed_ms = self.max_elapsed_ms.load(Ordering::Relaxed);
        let run_elapsed = (elapsed_ms > 0).then(|| std::time::Duration::from_millis(elapsed_ms));
        print!("{}", render(summary, run_elapsed));

        if !summary.threshold_violations.is_empty() {
            eprintln!("thresholds failed:");
            for v in &summary.threshold_violations {
                let key = if v.tags.is_empty() {
                    v.metric.clone()
                } else {
                    let selector = v
                        .tags
                        .iter()
                        .map(|(k, val)| format!("{k}={val}"))
                        .collect::<Vec<_>>()
                        .join(",");
                    format!("{}{{{selector}}}", v.metric)
                };
                match v.observed {
                    Some(obs) => eprintln!("  {key}: {} (observed {obs})", v.expression),
                    None => eprintln!("  {key}: {} (missing series)", v.expression),
                }
            }
        }

        Ok(())
    }
}
