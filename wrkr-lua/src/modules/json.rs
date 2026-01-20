use mlua::{Lua, Table, Value};

use crate::Result;
use crate::json_util;

pub(super) fn register(lua: &Lua) -> Result<()> {
    let loader = lua.create_function(|lua, ()| {
        let t = lua.create_table()?;

        let encode = lua.create_function(|lua, v: Value| {
            json_util::encode(lua, v).map_err(mlua::Error::external)
        })?;
        let decode = lua.create_function(|lua, s: String| {
            json_util::decode(lua, &s).map_err(mlua::Error::external)
        })?;

        t.set("encode", encode)?;
        t.set("decode", decode)?;
        Ok::<Table, mlua::Error>(t)
    })?;

    super::preload_set(lua, "wrkr/json", loader)
}
