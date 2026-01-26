use std::future::Future;
use std::pin::Pin;

use super::{RuntimeError, ScriptOutputs};

pub trait ScriptRuntime: Send + Sync {
    fn create_run_context(&self, env: &wrkr_core::EnvVars) -> wrkr_core::RunScenariosContext;

    fn parse_script_options(
        &self,
        run_ctx: &wrkr_core::RunScenariosContext,
    ) -> std::result::Result<wrkr_core::ScriptOptions, RuntimeError>;

    fn run_setup(
        &self,
        run_ctx: &wrkr_core::RunScenariosContext,
    ) -> std::result::Result<(), RuntimeError>;

    fn run_teardown(
        &self,
        run_ctx: &wrkr_core::RunScenariosContext,
    ) -> std::result::Result<(), RuntimeError>;

    fn run_handle_summary(
        &self,
        run_ctx: &wrkr_core::RunScenariosContext,
        summary: &wrkr_core::RunSummary,
    ) -> std::result::Result<Option<ScriptOutputs>, RuntimeError>;

    fn run_vu(
        &self,
        ctx: wrkr_core::VuContext,
    ) -> Pin<Box<dyn Future<Output = std::result::Result<(), RuntimeError>> + Send>>;
}
