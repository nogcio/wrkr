use std::sync::{Arc, Mutex};
use std::time::Duration;

mod duration;
mod progress;
use progress::HumanProgress;

use super::OutputFormatter;

pub(crate) struct HumanReadableOutput {}

impl HumanReadableOutput {
    pub(crate) fn new() -> Self {
        Self {}
    }
}

impl OutputFormatter for HumanReadableOutput {
    fn print_header(
        &self,
        script_path: &std::path::Path,
        scenarios: &[wrkr_core::runner::ScenarioConfig],
    ) {
    }

    fn progress(&self) -> Option<wrkr_core::runner::ProgressFn> {
        Some(Arc::new(move |u| {}))
    }

    fn print_summary(&self, summary: &wrkr_core::runner::RunSummary) -> anyhow::Result<()> {
        Ok(())
    }
}
