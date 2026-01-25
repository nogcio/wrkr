use std::sync::Arc;

use mlua::{Lua, Value};

use crate::value_util::{Int64Repr, value_to_lua};

pub(super) fn shared_value_to_lua(
    lua: &Lua,
    value: Option<Arc<wrkr_value::Value>>,
) -> mlua::Result<Value> {
    let Some(value) = value else {
        return Ok(Value::Nil);
    };

    value_to_lua(lua, &value, Int64Repr::Integer).map_err(mlua::Error::external)
}
