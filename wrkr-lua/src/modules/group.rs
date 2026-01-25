use mlua::{Function, Lua, Value};

use crate::Result;

const REG_CURRENT_GROUP: &str = "wrkr_current_group";

pub(super) fn current_group(lua: &Lua) -> Option<String> {
    lua.named_registry_value::<mlua::Value>(REG_CURRENT_GROUP)
        .ok()
        .and_then(|v| match v {
            mlua::Value::Nil => None,
            mlua::Value::String(s) => Some(s.to_string_lossy().to_string()),
            _ => None,
        })
}

fn set_current_group(lua: &Lua, group: Option<&str>) -> mlua::Result<()> {
    match group {
        None => lua.set_named_registry_value(REG_CURRENT_GROUP, mlua::Value::Nil),
        Some(g) => lua.set_named_registry_value(REG_CURRENT_GROUP, g),
    }
}

pub(super) fn register(lua: &Lua) -> Result<()> {
    let loader = lua.create_function(|lua, ()| {
        let t = lua.create_table()?;

        let group = lua.create_async_function(|lua, (name, f): (String, Function)| async move {
            let prev = current_group(&lua);
            set_current_group(&lua, Some(&name))?;

            let res: mlua::Result<Value> = f.call_async(()).await;

            // Always restore.
            match prev {
                None => set_current_group(&lua, None)?,
                Some(p) => set_current_group(&lua, Some(&p))?,
            }

            res
        })?;

        t.set("group", group)?;
        Ok::<mlua::Table, mlua::Error>(t)
    })?;

    super::preload_set(lua, "wrkr/group", loader)
}
