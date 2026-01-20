use std::collections::HashMap;
use std::sync::Arc;

use mlua::{Lua, Table, Value};

use crate::Result;

#[derive(Debug, Clone, Copy)]
pub enum Int64Repr {
    Integer,
    String,
}

pub fn lua_to_value(lua: &Lua, value: Value, int64_repr: Int64Repr) -> Result<wrkr_value::Value> {
    fn err(msg: &str) -> mlua::Error {
        mlua::Error::external(msg.to_string())
    }

    fn table_to_value(
        lua: &Lua,
        t: Table,
        depth: usize,
        int64_repr: Int64Repr,
    ) -> std::result::Result<wrkr_value::Value, mlua::Error> {
        if depth == 0 {
            return Err(err("value too deep"));
        }

        // First pass: detect a dense 1..=N array.
        let mut saw_non_int_key = false;
        let mut max_idx: usize = 0;
        let mut array_items: Vec<Option<wrkr_value::Value>> = Vec::new();

        // Also collect for object/map construction.
        let mut object_items: HashMap<Arc<str>, wrkr_value::Value> = HashMap::new();
        let mut map_items: HashMap<wrkr_value::MapKey, wrkr_value::Value> = HashMap::new();

        let mut saw_string_key = false;
        let mut saw_other_key = false;

        for pair in t.pairs::<Value, Value>() {
            let (k, v) = pair?;
            let v = lua_to_value_inner(lua, v, depth - 1, int64_repr)?;

            match k {
                Value::Integer(i) => {
                    if i >= 1 {
                        let idx = i as usize;
                        max_idx = max_idx.max(idx);
                        if array_items.len() < idx {
                            array_items.resize_with(idx, || None);
                        }
                        if array_items[idx - 1].is_some() {
                            return Err(err("duplicate array index"));
                        }
                        array_items[idx - 1] = Some(v.clone());

                        let mk = if i >= 0 {
                            wrkr_value::MapKey::U64(i as u64)
                        } else {
                            wrkr_value::MapKey::I64(i)
                        };
                        map_items.insert(mk, v);
                    } else {
                        saw_non_int_key = true;
                        saw_other_key = true;
                        map_items.insert(wrkr_value::MapKey::I64(i), v);
                    }
                }
                Value::String(s) => {
                    saw_non_int_key = true;
                    saw_string_key = true;
                    let key = Arc::<str>::from(s.to_string_lossy().to_string());
                    object_items.insert(key.clone(), v.clone());
                    map_items.insert(wrkr_value::MapKey::String(key), v);
                }
                Value::Boolean(b) => {
                    saw_non_int_key = true;
                    saw_other_key = true;
                    map_items.insert(wrkr_value::MapKey::Bool(b), v);
                }
                _ => {
                    saw_non_int_key = true;
                    saw_other_key = true;
                }
            }
        }

        // If this is a dense array with only integer keys, return Array.
        if !saw_non_int_key {
            if max_idx == 0 {
                return Ok(wrkr_value::Value::Array(Vec::new()));
            }
            if array_items.len() != max_idx || array_items.iter().any(Option::is_none) {
                return Err(err("sparse arrays are not supported"));
            }
            let mut out = Vec::with_capacity(array_items.len());
            for item in array_items {
                let Some(item) = item else {
                    return Err(err("sparse arrays are not supported"));
                };
                out.push(item);
            }
            return Ok(wrkr_value::Value::Array(out));
        }

        // Prefer Object if keys are only strings.
        if saw_string_key && !saw_other_key {
            return Ok(wrkr_value::Value::Object(object_items));
        }

        Ok(wrkr_value::Value::Map(map_items))
    }

    fn lua_to_value_inner(
        lua: &Lua,
        value: Value,
        depth: usize,
        int64_repr: Int64Repr,
    ) -> std::result::Result<wrkr_value::Value, mlua::Error> {
        if depth == 0 {
            return Err(err("value too deep"));
        }

        Ok(match value {
            Value::Nil => wrkr_value::Value::Null,
            Value::Boolean(v) => wrkr_value::Value::Bool(v),
            Value::Integer(v) => wrkr_value::Value::I64(v),
            Value::Number(v) => wrkr_value::Value::F64(v),
            Value::String(s) => {
                let s = s.to_string_lossy().to_string();
                wrkr_value::Value::String(Arc::<str>::from(s))
            }
            Value::Table(t) => table_to_value(lua, t, depth - 1, int64_repr)?,
            _ => return Err(err("unsupported value type")),
        })
    }

    Ok(lua_to_value_inner(lua, value, 64, int64_repr)?)
}

pub fn value_to_lua(lua: &Lua, value: &wrkr_value::Value, int64_repr: Int64Repr) -> Result<Value> {
    fn build(
        lua: &Lua,
        value: &wrkr_value::Value,
        depth: usize,
        int64_repr: Int64Repr,
    ) -> std::result::Result<Value, mlua::Error> {
        if depth == 0 {
            return Err(mlua::Error::external("value too deep"));
        }

        Ok(match value {
            wrkr_value::Value::Null => Value::Nil,
            wrkr_value::Value::Bool(v) => Value::Boolean(*v),
            wrkr_value::Value::I64(v) => match int64_repr {
                Int64Repr::Integer => Value::Integer(*v),
                Int64Repr::String => Value::String(lua.create_string(v.to_string().as_bytes())?),
            },
            wrkr_value::Value::U64(v) => match int64_repr {
                Int64Repr::Integer => {
                    if *v <= i64::MAX as u64 {
                        Value::Integer(*v as i64)
                    } else {
                        Value::String(lua.create_string(v.to_string().as_bytes())?)
                    }
                }
                Int64Repr::String => Value::String(lua.create_string(v.to_string().as_bytes())?),
            },
            wrkr_value::Value::F64(v) => Value::Number(*v),
            wrkr_value::Value::String(s) => {
                Value::String(lua.create_string(s.as_ref().as_bytes())?)
            }
            wrkr_value::Value::Bytes(b) => Value::String(lua.create_string(b.as_ref())?),
            wrkr_value::Value::Array(items) => {
                let t = lua.create_table_with_capacity(items.len(), 0)?;
                for (idx, item) in items.iter().enumerate() {
                    t.set(idx + 1, build(lua, item, depth - 1, int64_repr)?)?;
                }
                Value::Table(t)
            }
            wrkr_value::Value::Object(items) => {
                let t = lua.create_table_with_capacity(0, items.len())?;
                for (k, v) in items {
                    t.set(k.as_ref(), build(lua, v, depth - 1, int64_repr)?)?;
                }
                Value::Table(t)
            }
            wrkr_value::Value::Map(items) => {
                let t = lua.create_table_with_capacity(0, items.len())?;
                for (k, v) in items {
                    let lk = match k {
                        wrkr_value::MapKey::Bool(b) => Value::Boolean(*b),
                        wrkr_value::MapKey::I64(i) => match int64_repr {
                            Int64Repr::Integer => Value::Integer(*i),
                            Int64Repr::String => {
                                Value::String(lua.create_string(i.to_string().as_bytes())?)
                            }
                        },
                        wrkr_value::MapKey::U64(u) => match int64_repr {
                            Int64Repr::Integer => {
                                if *u <= i64::MAX as u64 {
                                    Value::Integer(*u as i64)
                                } else {
                                    Value::String(lua.create_string(u.to_string().as_bytes())?)
                                }
                            }
                            Int64Repr::String => {
                                Value::String(lua.create_string(u.to_string().as_bytes())?)
                            }
                        },
                        wrkr_value::MapKey::String(s) => {
                            Value::String(lua.create_string(s.as_ref().as_bytes())?)
                        }
                    };
                    t.set(lk, build(lua, v, depth - 1, int64_repr)?)?;
                }
                Value::Table(t)
            }
        })
    }

    Ok(build(lua, value, 64, int64_repr)?)
}
