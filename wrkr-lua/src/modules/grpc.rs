use std::path::Path;
use std::sync::Arc;

use mlua::Lua;

use crate::Result;
use crate::grpc_api::create_grpc_module;

pub(super) fn register_runtime(
    lua: &Lua,
    scenario: Arc<str>,
    script_path: Option<&Path>,
    stats: Arc<wrkr_core::runner::RunStats>,
) -> Result<()> {
    let script_path = script_path.map(|p| p.to_path_buf());
    let loader = {
        let scenario = scenario.clone();
        let stats = stats.clone();
        lua.create_function(move |lua, ()| {
            create_grpc_module(lua, scenario.clone(), script_path.as_deref(), stats.clone())
                .map_err(mlua::Error::external)
        })?
    };

    super::preload_set(lua, "wrkr/grpc", loader)
}
