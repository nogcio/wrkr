use std::sync::Arc;

use mlua::Lua;

use crate::Result;
use crate::http_api::create_http_module;

pub(super) fn register_runtime(lua: &Lua, client: Arc<wrkr_core::HttpClient>) -> Result<()> {
    let loader = {
        let client = client.clone();
        lua.create_function(move |lua, ()| {
            create_http_module(lua, client.clone()).map_err(mlua::Error::external)
        })?
    };
    super::preload_set(lua, "wrkr/http", loader)
}
