use std::sync::Arc;

mod duration;
mod progress;

use duration::format_duration_single;
use progress::HumanProgress;

use super::OutputFormatter;

pub(crate) struct HumanReadableOutput {
    progress: Arc<HumanProgress>,
}

impl HumanReadableOutput {
    pub(crate) fn new() -> Self {
        Self {
            progress: Arc::new(HumanProgress::new()),
        }
    }
}

impl OutputFormatter for HumanReadableOutput {
    fn print_header(
        &self,
        _script_path: &std::path::Path,
        _scenarios: &[wrkr_core::ScenarioConfig],
    ) {
    }

    fn progress(&self) -> Option<wrkr_core::ProgressFn> {
        let progress = self.progress.clone();
        Some(Arc::new(move |u| {
            let (total_duration_opt, message) = match &u.progress {
                wrkr_core::ScenarioProgress::ConstantVus { vus, duration } => (
                    *duration,
                    format!("vus={vus} elapsed={}", format_duration_single(u.elapsed)),
                ),
                wrkr_core::ScenarioProgress::RampingVus {
                    total_duration,
                    stage,
                } => {
                    let msg = if let Some(stage) = stage {
                        format!(
                            "stage={}/{} target={} elapsed={} stage_remaining={}",
                            stage.stage,
                            stage.stages,
                            stage.current_target,
                            format_duration_single(u.elapsed),
                            format_duration_single(stage.stage_remaining)
                        )
                    } else {
                        format!("elapsed={}", format_duration_single(u.elapsed))
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
                        "active_vus={active_vus}/{max_vus} dropped={dropped_iterations_total} elapsed={}",
                        format_duration_single(u.elapsed)
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

    fn print_summary(&self, _summary: &wrkr_core::RunSummary) -> anyhow::Result<()> {
        self.progress.finish();
        Ok(())
    }
}
