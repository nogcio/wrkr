use std::sync::{Arc, Mutex};
use std::time::Duration;

mod duration;
mod progress;
mod summary;
use duration::format_duration_single;
use progress::HumanProgress;
use summary::{format_duration_ms_opt, render_run_header, render_run_summary};

use super::OutputFormatter;

pub(crate) struct HumanReadableOutput {
    error_state: Arc<Mutex<ErrorTickState>>,
    progress: Arc<HumanProgress>,
}

impl HumanReadableOutput {
    pub(crate) fn new() -> Self {
        Self {
            error_state: Arc::new(Mutex::new(ErrorTickState {
                last_tick: 0,
                last_failed_total: 0,
                last_delta_failed: 0,
            })),
            progress: Arc::new(HumanProgress::new()),
        }
    }
}

impl OutputFormatter for HumanReadableOutput {
    fn print_header(
        &self,
        script_path: &std::path::Path,
        scenarios: &[wrkr_core::runner::ScenarioConfig],
    ) {
        render_run_header(script_path, scenarios);
    }

    fn progress(&self) -> Option<wrkr_core::runner::ProgressFn> {
        let error_state = self.error_state.clone();
        let progress = self.progress.clone();

        Some(Arc::new(move |u| {
            let iters = u.metrics.iterations_total;

            let (current_vus, vus_total_opt, total_duration_opt) =
                scenario_progress_totals(&u.progress);

            let scenario = u.scenario;
            let exec = u.exec;

            let (failed_total, delta_failed) =
                update_failed_request_state(&error_state, u.tick, u.metrics.failed_requests_total);
            let errors = format_failure_suffix(failed_total, delta_failed);
            let p95 = format_duration_ms_opt(u.metrics.latency_p95_ms_now);

            let vus_total = vus_total_opt.unwrap_or(current_vus);
            let header = format!("{exec}  {current_vus:03}/{vus_total:03} VUs  {iters} iters");
            let elapsed = format_duration_single(u.elapsed);

            let msg = match total_duration_opt {
                Some(total_d) => format!(
                    "{header}  {elapsed}/{}  rps={:.1}  lat(p95)={p95}{errors}",
                    format_duration_single(total_d),
                    u.metrics.rps_now,
                ),
                None => format!(
                    "{header}  {elapsed}  rps={:.1}  lat(p95)={p95}{errors}",
                    u.metrics.rps_now,
                ),
            };

            progress.update(&scenario, total_duration_opt, u.elapsed, msg);
        }))
    }

    fn print_summary(&self, summary: &wrkr_core::runner::RunSummary) -> anyhow::Result<()> {
        self.finish_progress_bars();
        render_run_summary(summary);
        Ok(())
    }
}

impl HumanReadableOutput {
    fn finish_progress_bars(&self) {
        self.progress.finish();
    }
}

#[derive(Debug, Clone, Copy)]
struct ErrorTickState {
    last_tick: u64,
    last_failed_total: u64,
    last_delta_failed: u64,
}

fn scenario_progress_totals(
    progress: &wrkr_core::runner::ScenarioProgress,
) -> (u64, Option<u64>, Option<Duration>) {
    match progress {
        wrkr_core::runner::ScenarioProgress::ConstantVus { vus, duration } => {
            (*vus, Some(*vus), *duration)
        }
        wrkr_core::runner::ScenarioProgress::RampingVus {
            total_duration,
            stage,
        } => {
            let current = stage.as_ref().map(|s| s.current_target).unwrap_or(0);
            let total = (!total_duration.is_zero()).then_some(*total_duration);
            (current, None, total)
        }
        wrkr_core::runner::ScenarioProgress::RampingArrivalRate {
            total_duration,
            active_vus,
            max_vus,
            ..
        } => {
            let total = (!total_duration.is_zero()).then_some(*total_duration);
            (*active_vus, Some(*max_vus), total)
        }
    }
}

fn update_failed_request_state(
    state: &Arc<Mutex<ErrorTickState>>,
    tick: u64,
    failed_total_now: u64,
) -> (u64, u64) {
    let mut st = state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if st.last_tick != tick {
        st.last_delta_failed = failed_total_now.saturating_sub(st.last_failed_total);
        st.last_failed_total = failed_total_now;
        st.last_tick = tick;
    }
    (st.last_failed_total, st.last_delta_failed)
}

fn format_failure_suffix(failed_total: u64, delta_failed: u64) -> String {
    if failed_total == 0 {
        String::new()
    } else if delta_failed == 0 {
        format!("  fails={failed_total}")
    } else {
        format!("  fails={failed_total} (+{delta_failed})")
    }
}
