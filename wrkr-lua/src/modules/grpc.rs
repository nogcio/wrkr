use std::path::Path;
use std::sync::Arc;

use mlua::Lua;

use crate::Result;
use crate::grpc_api::create_grpc_module;

pub(super) fn register_runtime(lua: &Lua, script_path: Option<&Path>, max_vus: u64) -> Result<()> {
    let script_path = script_path.map(|p| p.to_path_buf());
    let loader = {
        lua.create_function(move |lua, ()| {
            create_grpc_module(lua, script_path.as_deref(), max_vus).map_err(mlua::Error::external)
        })?
    };

    super::preload_set(lua, "wrkr/grpc", loader)
}
