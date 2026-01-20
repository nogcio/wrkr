use mlua::{Lua, Value};

use crate::Result;

pub fn encode(_lua: &Lua, value: Value) -> Result<String> {
    // Transcode from Lua's serde Deserializer straight into serde_json's Serializer,
    // avoiding an intermediate serde_json::Value allocation tree.
    let mut out = Vec::with_capacity(256);
    let mut serializer = serde_json::Serializer::new(&mut out);
    let deserializer = mlua::serde::de::Deserializer::new(value);
    serde_transcode::transcode(deserializer, &mut serializer).map_err(mlua::Error::external)?;

    // serde_json always emits UTF-8.
    let s = String::from_utf8(out).map_err(mlua::Error::external)?;
    Ok(s)
}

pub fn decode(lua: &Lua, s: &str) -> Result<Value> {
    // Transcode from JSON directly into Lua values via mlua's serde Serializer.
    let mut deserializer = serde_json::Deserializer::from_str(s);
    let serializer = mlua::serde::ser::Serializer::new(lua);
    let v =
        serde_transcode::transcode(&mut deserializer, serializer).map_err(mlua::Error::external)?;
    Ok(v)
}
