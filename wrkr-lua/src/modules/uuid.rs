use mlua::{Lua, Table};

use crate::Result;

pub(super) fn register(lua: &Lua) -> Result<()> {
    let loader = lua.create_function(|lua, ()| {
        let t = lua.create_table()?;

        let v4 = lua.create_function(|lua, ()| {
            let s = uuid::Uuid::new_v4().to_string();
            lua.create_string(s.as_bytes())
        })?;

        t.set("v4", v4)?;
        Ok::<Table, mlua::Error>(t)
    })?;

    super::preload_set(lua, "wrkr/uuid", loader)
}
