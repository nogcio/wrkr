use crate::cli::OutputFormat;
use std::path::Path;

mod human;
mod json;

pub(crate) trait OutputFormatter: Send + Sync {
    fn print_header(&self, script_path: &Path, scenarios: &[wrkr_core::runner::ScenarioConfig]);
    fn progress(&self) -> Option<wrkr_core::runner::ProgressFn>;
    fn print_summary(&self, summary: &wrkr_core::runner::RunSummary) -> anyhow::Result<()>;
}

pub(crate) fn formatter(format: OutputFormat) -> Box<dyn OutputFormatter> {
    match format {
        OutputFormat::HumanReadable => Box::new(human::HumanReadableOutput::new()),
        OutputFormat::Json => Box::new(json::JsonOutput),
    }
}
