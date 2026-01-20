use std::sync::Arc;

use mlua::Lua;

use crate::Result;
use crate::metrics_api::create_metrics_module;

pub(super) fn register_runtime(lua: &Lua, stats: Arc<wrkr_core::runner::RunStats>) -> Result<()> {
    let loader = {
        let stats = stats.clone();
        lua.create_function(move |lua, ()| {
            create_metrics_module(lua, stats.clone()).map_err(mlua::Error::external)
        })?
    };
    super::preload_set(lua, "wrkr/metrics", loader)
}
