use mlua::{Function, Lua, Value};

use crate::Result;
use crate::group_api;

pub(super) fn register(lua: &Lua) -> Result<()> {
    let loader = lua.create_function(|lua, ()| {
        let t = lua.create_table()?;

        let group = lua.create_async_function(|lua, (name, f): (String, Function)| async move {
            let prev = group_api::current_group(&lua);
            group_api::set_current_group(&lua, Some(&name))?;

            let res: mlua::Result<Value> = f.call_async(()).await;

            // Always restore.
            match prev {
                None => group_api::set_current_group(&lua, None)?,
                Some(p) => group_api::set_current_group(&lua, Some(&p))?,
            }

            res
        })?;

        t.set("group", group)?;
        Ok::<mlua::Table, mlua::Error>(t)
    })?;

    super::preload_set(lua, "wrkr/group", loader)
}
