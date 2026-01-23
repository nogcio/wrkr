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

        // Track whether we have *only* positive integer keys (1..=N), which allows Array.
        // We build the array incrementally and only validate contiguity at the end.
        let mut only_pos_int_keys = true;
        let mut max_idx: usize = 0;
        let mut array_values: Vec<Option<wrkr_value::Value>> = Vec::new();

        // Prefer Object when keys are only strings. If we see any other key type, we
        // lazily upgrade to Map (and migrate any accumulated Object/Array entries).
        let mut saw_string_key = false;
        let mut saw_other_key = false;

        let mut object_items: Option<wrkr_value::ObjectMap> = Some(wrkr_value::ObjectMap::new());
        let mut map_items: Option<wrkr_value::MapMap> = None;

        fn ensure_map<'a>(
            map_items: &'a mut Option<wrkr_value::MapMap>,
            object_items: &mut Option<wrkr_value::ObjectMap>,
            array_values: &mut [Option<wrkr_value::Value>],
        ) -> std::result::Result<&'a mut wrkr_value::MapMap, mlua::Error> {
            if map_items.is_none() {
                let mut m = wrkr_value::MapMap::new();

                if let Some(obj) = object_items.take() {
                    m.reserve(obj.len());
                    for (k, v) in obj {
                        m.insert(wrkr_value::MapKey::String(k), v);
                    }
                }

                if !array_values.is_empty() {
                    m.reserve(array_values.len());
                    for (idx, v) in array_values.iter_mut().enumerate() {
                        if let Some(v) = v.take() {
                            m.insert(wrkr_value::MapKey::U64((idx + 1) as u64), v);
                        }
                    }
                }

                *map_items = Some(m);
            }

            match map_items.as_mut() {
                Some(m) => Ok(m),
                None => Err(err("internal: failed to initialize map")),
            }
        }

        for pair in t.pairs::<Value, Value>() {
            let (k, v) = pair?;
            let v = lua_to_value_inner(lua, v, depth - 1, int64_repr)?;

            match k {
                Value::Integer(i) => {
                    saw_other_key = true;

                    if i >= 1 && only_pos_int_keys && map_items.is_none() {
                        let idx = i as usize;
                        max_idx = max_idx.max(idx);
                        if array_values.len() < idx {
                            array_values.resize_with(idx, || None);
                        }
                        if array_values[idx - 1].is_some() {
                            return Err(err("duplicate array index"));
                        }
                        array_values[idx - 1] = Some(v);
                    } else {
                        if i < 1 {
                            only_pos_int_keys = false;
                        }

                        let m = ensure_map(&mut map_items, &mut object_items, &mut array_values)?;
                        let mk = if i >= 0 {
                            wrkr_value::MapKey::U64(i as u64)
                        } else {
                            wrkr_value::MapKey::I64(i)
                        };
                        m.insert(mk, v);
                    }
                }
                Value::String(s) => {
                    saw_string_key = true;

                    // Once we see a string key, this is not an Array.
                    only_pos_int_keys = false;

                    let key = match s.to_str() {
                        Ok(st) => Arc::<str>::from(st.as_ref()),
                        Err(_) => Arc::<str>::from(s.to_string_lossy().to_string()),
                    };

                    let had_array_values = !array_values.is_empty();
                    if let Some(m) = map_items.as_mut() {
                        m.insert(wrkr_value::MapKey::String(key), v);
                    } else if had_array_values {
                        // We started as an Array but then encountered a string key; upgrade.
                        let m = ensure_map(&mut map_items, &mut object_items, &mut array_values)?;
                        m.insert(wrkr_value::MapKey::String(key), v);
                    } else if let Some(obj) = object_items.as_mut() {
                        obj.insert(key, v);
                    } else {
                        return Err(err("internal: object map missing"));
                    }
                }
                Value::Boolean(b) => {
                    saw_other_key = true;
                    only_pos_int_keys = false;

                    let m = ensure_map(&mut map_items, &mut object_items, &mut array_values)?;
                    m.insert(wrkr_value::MapKey::Bool(b), v);
                }
                _ => {
                    // Unsupported key types are ignored for value storage, but they do
                    // affect array/object detection (matches previous behavior).
                    saw_other_key = true;
                    only_pos_int_keys = false;
                }
            }
        }

        if let Some(map_items) = map_items {
            return Ok(wrkr_value::Value::Map(map_items));
        }

        if only_pos_int_keys {
            if max_idx == 0 {
                return Ok(wrkr_value::Value::Array(Vec::new()));
            }
            if array_values.len() != max_idx || array_values.iter().any(Option::is_none) {
                return Err(err("sparse arrays are not supported"));
            }

            let mut out = Vec::with_capacity(max_idx);
            for v in array_values {
                let Some(v) = v else {
                    return Err(err("sparse arrays are not supported"));
                };
                out.push(v);
            }
            return Ok(wrkr_value::Value::Array(out));
        }

        if saw_string_key && !saw_other_key {
            return Ok(wrkr_value::Value::Object(object_items.unwrap_or_default()));
        }

        // Mixed/unsupported keys but no stored entries.
        Ok(wrkr_value::Value::Map(wrkr_value::MapMap::new()))
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
                let s = match s.to_str() {
                    Ok(st) => Arc::<str>::from(st.as_ref()),
                    Err(_) => Arc::<str>::from(s.to_string_lossy().to_string()),
                };
                wrkr_value::Value::String(s)
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
