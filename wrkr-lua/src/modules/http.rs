use std::sync::Arc;

use mlua::Lua;

use crate::Result;
use crate::http_api::create_http_module;

pub(super) fn register_runtime(
    lua: &Lua,
    scenario: Arc<str>,
    client: Arc<wrkr_core::HttpClient>,
    stats: Arc<wrkr_core::runner::RunStats>,
) -> Result<()> {
    let loader = {
        let scenario = scenario.clone();
        let client = client.clone();
        let stats = stats.clone();
        lua.create_function(move |lua, ()| {
            create_http_module(lua, scenario.clone(), client.clone(), stats.clone())
                .map_err(mlua::Error::external)
        })?
    };
    super::preload_set(lua, "wrkr/http", loader)
}
