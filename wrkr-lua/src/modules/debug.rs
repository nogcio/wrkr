use mlua::{Lua, Table};

use crate::Result;
use crate::debugger;

pub(super) fn register(lua: &Lua) -> Result<()> {
    let loader = lua.create_function(|lua, ()| {
        let t = lua.create_table()?;

        let start = lua.create_function(|lua, ()| {
            debugger::start_debugger(lua).map_err(mlua::Error::external)
        })?;
        t.set("start", start)?;

        let maybe_start = lua.create_function(|lua, ()| {
            debugger::maybe_start_debugger(lua);
            Ok::<(), mlua::Error>(())
        })?;
        t.set("maybe_start", maybe_start)?;

        Ok::<Table, mlua::Error>(t)
    })?;

    super::preload_set(lua, "wrkr/debug", loader)
}
