use mlua::{Lua, Value};

use crate::value_util::{Int64Repr, lua_to_value};

pub(super) struct SharedSetLuaArgs {
    pub(super) key: String,
    pub(super) value: wrkr_value::Value,
}

impl SharedSetLuaArgs {
    pub(super) fn parse(lua: &Lua, key: String, value: Value) -> mlua::Result<Self> {
        let value = lua_to_value(lua, value, Int64Repr::Integer).map_err(mlua::Error::external)?;
        Ok(Self { key, value })
    }
}
