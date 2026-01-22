use std::sync::Arc;

use mlua::Lua;

use crate::Result;
use crate::http_api::create_check_function;

pub(super) fn register_runtime(
    lua: &Lua,
    scenario: Arc<str>,
    stats: Arc<wrkr_core::runner::RunStats>,
) -> Result<()> {
    let loader = {
        let scenario = scenario.clone();
        let stats = stats.clone();
        lua.create_function(move |lua, ()| {
            let f = create_check_function(lua, scenario.clone(), stats.clone())
                .map_err(mlua::Error::external)?;
            Ok::<mlua::Value, mlua::Error>(mlua::Value::Function(f))
        })?
    };
    super::preload_set(lua, "wrkr/check", loader)
}
