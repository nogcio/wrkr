use mlua::Lua;

const REG_CURRENT_GROUP: &str = "wrkr_current_group";

pub fn current_group(lua: &Lua) -> Option<String> {
    lua.named_registry_value::<mlua::Value>(REG_CURRENT_GROUP)
        .ok()
        .and_then(|v| match v {
            mlua::Value::Nil => None,
            mlua::Value::String(s) => Some(s.to_string_lossy().to_string()),
            _ => None,
        })
}

pub fn set_current_group(lua: &Lua, group: Option<&str>) -> mlua::Result<()> {
    match group {
        None => lua.set_named_registry_value(REG_CURRENT_GROUP, mlua::Value::Nil),
        Some(g) => lua.set_named_registry_value(REG_CURRENT_GROUP, g),
    }
}
