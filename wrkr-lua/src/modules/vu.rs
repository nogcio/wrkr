use mlua::{Lua, Table};

use crate::Result;

pub(super) fn register(lua: &Lua, vu_id: u64) -> Result<()> {
    let loader = lua.create_function(move |lua, ()| {
        let t = lua.create_table()?;
        let id = lua.create_function(move |_lua, ()| Ok(vu_id))?;
        t.set("id", id)?;
        Ok::<Table, mlua::Error>(t)
    })?;

    super::preload_set(lua, "wrkr/vu", loader)
}
