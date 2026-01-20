use mlua::{Lua, Table, Value};
use std::collections::HashMap;
use std::sync::Arc;

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

pub fn to_shared_value(lua: &Lua, value: Value) -> Result<wrkr_core::runner::SharedValue> {
    fn err(msg: &str) -> mlua::Error {
        mlua::Error::external(format!("shared.set: {msg}"))
    }

    fn convert_table(
        lua: &Lua,
        t: Table,
        depth: usize,
    ) -> std::result::Result<wrkr_core::runner::SharedValue, mlua::Error> {
        if depth == 0 {
            return Err(err("value too deep"));
        }

        let mut array_items: Vec<Option<wrkr_core::runner::SharedValue>> = Vec::new();
        let mut object_items: HashMap<Arc<str>, wrkr_core::runner::SharedValue> = HashMap::new();

        let mut saw_int_key = false;
        let mut saw_string_key = false;

        for pair in t.pairs::<Value, Value>() {
            let (k, v) = pair?;
            let v = to_shared_value_inner(lua, v, depth - 1)?;

            match k {
                Value::Integer(i) => {
                    if i < 1 {
                        return Err(err("array keys must be >= 1"));
                    }
                    let idx = i as usize;
                    saw_int_key = true;
                    if array_items.len() < idx {
                        array_items.resize_with(idx, || None);
                    }
                    if array_items[idx - 1].is_some() {
                        return Err(err("duplicate array index"));
                    }
                    array_items[idx - 1] = Some(v);
                }
                Value::String(s) => {
                    saw_string_key = true;
                    let key = Arc::<str>::from(s.to_string_lossy().to_string());
                    object_items.insert(key, v);
                }
                _ => return Err(err("table keys must be strings or integers")),
            }
        }

        match (saw_int_key, saw_string_key) {
            (true, true) => Err(err(
                "mixed table keys are not supported (use pure array or pure object)",
            )),
            (true, false) => {
                if array_items.iter().any(Option::is_none) {
                    return Err(err("sparse arrays are not supported"));
                }
                let mut out = Vec::with_capacity(array_items.len());
                for item in array_items {
                    let Some(item) = item else {
                        return Err(err("sparse arrays are not supported"));
                    };
                    out.push(item);
                }
                Ok(wrkr_core::runner::SharedValue::Array(out))
            }
            (false, true) => Ok(wrkr_core::runner::SharedValue::Object(object_items)),
            (false, false) => Ok(wrkr_core::runner::SharedValue::Object(HashMap::new())),
        }
    }

    fn to_shared_value_inner(
        lua: &Lua,
        value: Value,
        depth: usize,
    ) -> std::result::Result<wrkr_core::runner::SharedValue, mlua::Error> {
        if depth == 0 {
            return Err(err("value too deep"));
        }

        match value {
            Value::Nil => Ok(wrkr_core::runner::SharedValue::Null),
            Value::Boolean(v) => Ok(wrkr_core::runner::SharedValue::Bool(v)),
            Value::Integer(v) => Ok(wrkr_core::runner::SharedValue::I64(v)),
            Value::Number(v) => Ok(wrkr_core::runner::SharedValue::F64(v)),
            Value::String(s) => Ok(wrkr_core::runner::SharedValue::String(Arc::<str>::from(
                s.to_string_lossy().to_string(),
            ))),
            Value::Table(t) => convert_table(lua, t, depth - 1),
            _ => Err(err("unsupported value type")),
        }
    }

    Ok(to_shared_value_inner(lua, value, 64)?)
}

pub fn from_shared_value(lua: &Lua, value: &wrkr_core::runner::SharedValue) -> Result<Value> {
    fn build(
        lua: &Lua,
        value: &wrkr_core::runner::SharedValue,
        depth: usize,
    ) -> std::result::Result<Value, mlua::Error> {
        if depth == 0 {
            return Err(mlua::Error::external("shared.get: value too deep"));
        }

        match value {
            wrkr_core::runner::SharedValue::Null => Ok(Value::Nil),
            wrkr_core::runner::SharedValue::Bool(v) => Ok(Value::Boolean(*v)),
            wrkr_core::runner::SharedValue::I64(v) => Ok(Value::Integer(*v)),
            wrkr_core::runner::SharedValue::F64(v) => Ok(Value::Number(*v)),
            wrkr_core::runner::SharedValue::String(s) => {
                Ok(Value::String(lua.create_string(s.as_ref())?))
            }
            wrkr_core::runner::SharedValue::Array(items) => {
                let t = lua.create_table_with_capacity(items.len(), 0)?;
                for (idx, item) in items.iter().enumerate() {
                    let v = build(lua, item, depth - 1)?;
                    t.set(idx + 1, v)?;
                }
                Ok(Value::Table(t))
            }
            wrkr_core::runner::SharedValue::Object(items) => {
                let t = lua.create_table_with_capacity(0, items.len())?;
                for (k, v) in items {
                    let v = build(lua, v, depth - 1)?;
                    t.set(k.as_ref(), v)?;
                }
                Ok(Value::Table(t))
            }
        }
    }

    Ok(build(lua, value, 64)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok<T>(r: Result<T>) -> T {
        match r {
            Ok(v) => v,
            Err(err) => panic!("unexpected error: {err}"),
        }
    }

    fn ok_lua<T>(r: mlua::Result<T>) -> T {
        match r {
            Ok(v) => v,
            Err(err) => panic!("unexpected lua error: {err}"),
        }
    }

    fn some<T>(v: Option<T>, context: &str) -> T {
        match v {
            Some(v) => v,
            None => panic!("expected Some: {context}"),
        }
    }

    #[test]
    fn lua_table_array_roundtrip() {
        let lua = Lua::new();
        let t = ok_lua(lua.create_table());
        ok_lua(t.set(1, 10));
        ok_lua(t.set(2, 20));

        let sv = ok(to_shared_value(&lua, Value::Table(t)));
        assert_eq!(
            sv,
            wrkr_core::runner::SharedValue::Array(vec![
                wrkr_core::runner::SharedValue::I64(10),
                wrkr_core::runner::SharedValue::I64(20),
            ])
        );

        let v = ok(from_shared_value(&lua, &sv));
        let Value::Table(t2) = v else {
            panic!("expected table");
        };
        assert_eq!(ok_lua(t2.get::<i64>(1)), 10);
        assert_eq!(ok_lua(t2.get::<i64>(2)), 20);
    }

    #[test]
    fn lua_table_object_roundtrip() {
        let lua = Lua::new();
        let t = ok_lua(lua.create_table());
        ok_lua(t.set("a", true));
        ok_lua(t.set("b", "x"));

        let sv = ok(to_shared_value(&lua, Value::Table(t)));
        let wrkr_core::runner::SharedValue::Object(map) = sv else {
            panic!("expected object");
        };

        assert_eq!(
            some(map.get("a"), "missing key a"),
            &wrkr_core::runner::SharedValue::Bool(true)
        );
        assert_eq!(
            some(map.get("b"), "missing key b"),
            &wrkr_core::runner::SharedValue::String(Arc::<str>::from("x"))
        );
    }

    #[test]
    fn rejects_mixed_keys() {
        let lua = Lua::new();
        let t = ok_lua(lua.create_table());
        ok_lua(t.set(1, 1));
        ok_lua(t.set("x", 2));

        let msg = match to_shared_value(&lua, Value::Table(t)) {
            Ok(_) => panic!("expected error"),
            Err(err) => err.to_string(),
        };
        assert!(msg.contains("mixed table keys"), "unexpected error: {msg}");
    }

    #[test]
    fn rejects_sparse_arrays() {
        let lua = Lua::new();
        let t = ok_lua(lua.create_table());
        ok_lua(t.set(1, 1));
        ok_lua(t.set(3, 3));

        let msg = match to_shared_value(&lua, Value::Table(t)) {
            Ok(_) => panic!("expected error"),
            Err(err) => err.to_string(),
        };
        assert!(msg.contains("sparse arrays"), "unexpected error: {msg}");
    }
}
