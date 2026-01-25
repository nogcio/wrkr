use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;

use super::{RuntimeError, ScriptOutputs, ScriptRuntime};

pub struct LuaRuntime {
    script: String,
    script_path: PathBuf,
}

impl LuaRuntime {
    pub fn new(path: &Path, script: String) -> anyhow::Result<Self> {
        Ok(Self {
            script,
            script_path: path.to_path_buf(),
        })
    }
}

impl ScriptRuntime for LuaRuntime {
    fn create_run_context(&self, env: &wrkr_core::EnvVars) -> wrkr_core::RunScenariosContext {
        wrkr_core::RunScenariosContext::new(
            env.clone(),
            self.script.clone(),
            self.script_path.clone(),
        )
    }
    fn parse_script_options(
        &self,
        run_ctx: &wrkr_core::RunScenariosContext,
    ) -> std::result::Result<wrkr_core::ScriptOptions, RuntimeError> {
        wrkr_lua::parse_script_options(run_ctx).map_err(RuntimeError::from)
    }

    fn run_setup(
        &self,
        run_ctx: &wrkr_core::RunScenariosContext,
    ) -> std::result::Result<(), RuntimeError> {
        wrkr_lua::run_setup(run_ctx).map_err(RuntimeError::from)
    }

    fn run_teardown(
        &self,
        run_ctx: &wrkr_core::RunScenariosContext,
    ) -> std::result::Result<(), RuntimeError> {
        wrkr_lua::run_teardown(run_ctx).map_err(RuntimeError::from)
    }

    fn run_handle_summary(
        &self,
        run_ctx: &wrkr_core::RunScenariosContext,
    ) -> std::result::Result<Option<ScriptOutputs>, RuntimeError> {
        let out = wrkr_lua::run_handle_summary(run_ctx).map_err(RuntimeError::from)?;

        Ok(out.map(|o| ScriptOutputs {
            stdout: o.stdout,
            stderr: o.stderr,
            files: o.files,
        }))
    }

    fn run_vu(
        &self,
        ctx: wrkr_core::VuContext,
    ) -> Pin<Box<dyn Future<Output = std::result::Result<(), RuntimeError>> + Send>> {
        Box::pin(async move { wrkr_lua::run_vu(ctx).await.map_err(RuntimeError::from) })
    }
}
