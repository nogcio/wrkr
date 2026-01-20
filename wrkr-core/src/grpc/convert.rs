use std::collections::HashMap;
use std::sync::Arc;

use prost_reflect::ReflectMessage;

pub(super) fn value_to_dynamic_message(
    msg_desc: prost_reflect::MessageDescriptor,
    value: wrkr_value::Value,
) -> std::result::Result<prost_reflect::DynamicMessage, String> {
    let mut msg = prost_reflect::DynamicMessage::new(msg_desc.clone());

    let fields = match value {
        wrkr_value::Value::Object(map) => map,
        wrkr_value::Value::Map(map) => {
            let mut out = HashMap::with_capacity(map.len());
            for (k, v) in map {
                let wrkr_value::MapKey::String(key) = k else {
                    return Err(format!(
                        "message {} expects string field names",
                        msg_desc.full_name()
                    ));
                };
                out.insert(key, v);
            }
            out
        }
        _ => {
            return Err(format!(
                "message {} must be an object",
                msg_desc.full_name()
            ));
        }
    };

    for (field_name, v) in fields {
        let Some(field) = msg_desc.get_field_by_name(&field_name) else {
            return Err(format!(
                "unknown field '{field_name}' for message {}",
                msg_desc.full_name()
            ));
        };

        let pv = value_to_proto_value(&field, v)?;
        msg.set_field(&field, pv);
    }

    Ok(msg)
}

fn value_to_proto_value(
    field: &prost_reflect::FieldDescriptor,
    value: wrkr_value::Value,
) -> std::result::Result<prost_reflect::Value, String> {
    let kind = field.kind();

    if field.is_list() {
        let wrkr_value::Value::Array(items) = value else {
            return Err(format!("field '{}' must be an array", field.name()));
        };

        let mut out: Vec<prost_reflect::Value> = Vec::with_capacity(items.len());
        for item in items {
            out.push(value_to_proto_scalar_value(kind.clone(), item)?);
        }
        return Ok(prost_reflect::Value::List(out));
    }

    if field.is_map() {
        let prost_reflect::Kind::Message(entry_desc) = field.kind() else {
            return Err("expected map field".to_string());
        };

        let key_kind = entry_desc
            .get_field_by_name("key")
            .ok_or_else(|| "invalid map entry: missing key".to_string())?
            .kind();
        let value_kind = entry_desc
            .get_field_by_name("value")
            .ok_or_else(|| "invalid map entry: missing value".to_string())?
            .kind();

        let entries: Vec<(wrkr_value::MapKey, wrkr_value::Value)> = match value {
            wrkr_value::Value::Object(m) => m
                .into_iter()
                .map(|(k, v)| (wrkr_value::MapKey::String(k), v))
                .collect(),
            wrkr_value::Value::Map(m) => m.into_iter().collect(),
            _ => return Err(format!("field '{}' must be a map/object", field.name())),
        };

        let mut out: std::collections::HashMap<prost_reflect::MapKey, prost_reflect::Value> =
            std::collections::HashMap::new();

        for (k, v) in entries {
            let pk = map_key_to_proto(key_kind.clone(), k)?;
            let pv = value_to_proto_scalar_value(value_kind.clone(), v)?;
            out.insert(pk, pv);
        }

        return Ok(prost_reflect::Value::Map(out));
    }

    value_to_proto_scalar_value(kind, value)
}

fn map_key_to_proto(
    kind: prost_reflect::Kind,
    key: wrkr_value::MapKey,
) -> std::result::Result<prost_reflect::MapKey, String> {
    Ok(match kind {
        prost_reflect::Kind::String => match key {
            wrkr_value::MapKey::String(s) => prost_reflect::MapKey::String(s.to_string()),
            _ => return Err("map key must be a string".to_string()),
        },
        prost_reflect::Kind::Int32
        | prost_reflect::Kind::Sint32
        | prost_reflect::Kind::Sfixed32 => {
            let n = map_key_to_i64(key)?;
            prost_reflect::MapKey::I32(n as i32)
        }
        prost_reflect::Kind::Int64
        | prost_reflect::Kind::Sint64
        | prost_reflect::Kind::Sfixed64 => prost_reflect::MapKey::I64(map_key_to_i64(key)?),
        prost_reflect::Kind::Uint32 | prost_reflect::Kind::Fixed32 => {
            let n = map_key_to_u64(key)?;
            prost_reflect::MapKey::U32(n as u32)
        }
        prost_reflect::Kind::Uint64 | prost_reflect::Kind::Fixed64 => {
            prost_reflect::MapKey::U64(map_key_to_u64(key)?)
        }
        prost_reflect::Kind::Bool => match key {
            wrkr_value::MapKey::Bool(b) => prost_reflect::MapKey::Bool(b),
            _ => return Err("map key must be boolean".to_string()),
        },
        _ => return Err("unsupported map key kind".to_string()),
    })
}

fn map_key_to_i64(key: wrkr_value::MapKey) -> std::result::Result<i64, String> {
    match key {
        wrkr_value::MapKey::I64(i) => Ok(i),
        wrkr_value::MapKey::U64(u) => Ok(u as i64),
        wrkr_value::MapKey::String(s) => s
            .parse::<i64>()
            .map_err(|_| "invalid int64 map key".to_string()),
        _ => Err("invalid int64 map key".to_string()),
    }
}

fn map_key_to_u64(key: wrkr_value::MapKey) -> std::result::Result<u64, String> {
    match key {
        wrkr_value::MapKey::U64(u) => Ok(u),
        wrkr_value::MapKey::I64(i) if i >= 0 => Ok(i as u64),
        wrkr_value::MapKey::String(s) => s
            .parse::<u64>()
            .map_err(|_| "invalid uint64 map key".to_string()),
        _ => Err("invalid uint64 map key".to_string()),
    }
}

fn value_to_proto_scalar_value(
    kind: prost_reflect::Kind,
    value: wrkr_value::Value,
) -> std::result::Result<prost_reflect::Value, String> {
    use prost_reflect::Value as V;

    Ok(match kind {
        prost_reflect::Kind::Bool => V::Bool(match value {
            wrkr_value::Value::Bool(b) => b,
            wrkr_value::Value::String(s) => matches!(s.as_ref(), "true" | "1"),
            wrkr_value::Value::I64(i) => i != 0,
            wrkr_value::Value::U64(u) => u != 0,
            _ => false,
        }),
        prost_reflect::Kind::String => match value {
            wrkr_value::Value::String(s) => V::String(s.to_string()),
            wrkr_value::Value::I64(i) => V::String(i.to_string()),
            wrkr_value::Value::U64(u) => V::String(u.to_string()),
            wrkr_value::Value::F64(f) => V::String(f.to_string()),
            wrkr_value::Value::Bool(b) => V::String(b.to_string()),
            _ => V::String(String::new()),
        },
        prost_reflect::Kind::Bytes => match value {
            wrkr_value::Value::Bytes(b) => V::Bytes(b.to_vec().into()),
            wrkr_value::Value::String(s) => V::Bytes(s.as_ref().as_bytes().to_vec().into()),
            _ => V::Bytes(bytes::Bytes::new()),
        },

        prost_reflect::Kind::Int32
        | prost_reflect::Kind::Sint32
        | prost_reflect::Kind::Sfixed32 => V::I32(to_i64(value)? as i32),
        prost_reflect::Kind::Int64
        | prost_reflect::Kind::Sint64
        | prost_reflect::Kind::Sfixed64 => V::I64(to_i64(value)?),
        prost_reflect::Kind::Uint32 | prost_reflect::Kind::Fixed32 => V::U32(to_u64(value)? as u32),
        prost_reflect::Kind::Uint64 | prost_reflect::Kind::Fixed64 => V::U64(to_u64(value)?),

        prost_reflect::Kind::Float => V::F32(to_f64(value)? as f32),
        prost_reflect::Kind::Double => V::F64(to_f64(value)?),

        prost_reflect::Kind::Enum(enum_desc) => match value {
            wrkr_value::Value::String(s) => {
                if let Some(v) = enum_desc.get_value_by_name(s.as_ref()) {
                    V::EnumNumber(v.number())
                } else {
                    V::EnumNumber(to_i64(wrkr_value::Value::String(s))? as i32)
                }
            }
            other => V::EnumNumber(to_i64(other)? as i32),
        },

        prost_reflect::Kind::Message(msg_desc) => {
            let msg = value_to_dynamic_message(msg_desc, value)?;
            V::Message(msg)
        }
    })
}

fn to_i64(value: wrkr_value::Value) -> std::result::Result<i64, String> {
    match value {
        wrkr_value::Value::I64(i) => Ok(i),
        wrkr_value::Value::U64(u) => Ok(u as i64),
        wrkr_value::Value::F64(f) => Ok(f as i64),
        wrkr_value::Value::String(s) => s
            .parse::<i64>()
            .map_err(|_| "invalid int64 string".to_string()),
        _ => Err("invalid integer".to_string()),
    }
}

fn to_u64(value: wrkr_value::Value) -> std::result::Result<u64, String> {
    match value {
        wrkr_value::Value::U64(u) => Ok(u),
        wrkr_value::Value::I64(i) if i >= 0 => Ok(i as u64),
        wrkr_value::Value::F64(f) if f >= 0.0 => Ok(f as u64),
        wrkr_value::Value::String(s) => s
            .parse::<u64>()
            .map_err(|_| "invalid uint64 string".to_string()),
        _ => Err("invalid unsigned integer".to_string()),
    }
}

fn to_f64(value: wrkr_value::Value) -> std::result::Result<f64, String> {
    match value {
        wrkr_value::Value::F64(f) => Ok(f),
        wrkr_value::Value::I64(i) => Ok(i as f64),
        wrkr_value::Value::U64(u) => Ok(u as f64),
        wrkr_value::Value::String(s) => s
            .parse::<f64>()
            .map_err(|_| "invalid float string".to_string()),
        _ => Err("invalid number".to_string()),
    }
}

pub(super) fn dynamic_message_to_value(msg: &prost_reflect::DynamicMessage) -> wrkr_value::Value {
    let desc = msg.descriptor();
    let mut out: HashMap<Arc<str>, wrkr_value::Value> = HashMap::new();

    for field in desc.fields() {
        if !msg.has_field(&field) {
            continue;
        }

        let v = msg.get_field(&field);
        out.insert(
            Arc::<str>::from(field.name().to_string()),
            proto_value_to_value(&v),
        );
    }

    wrkr_value::Value::Object(out)
}

fn proto_value_to_value(v: &prost_reflect::Value) -> wrkr_value::Value {
    use prost_reflect::Value as V;

    match v {
        V::Bool(b) => wrkr_value::Value::Bool(*b),
        V::I32(i) => wrkr_value::Value::I64(i64::from(*i)),
        V::I64(i) => wrkr_value::Value::I64(*i),
        V::U32(u) => wrkr_value::Value::U64(u64::from(*u)),
        V::U64(u) => wrkr_value::Value::U64(*u),
        V::F32(f) => wrkr_value::Value::F64(f64::from(*f)),
        V::F64(f) => wrkr_value::Value::F64(*f),
        V::String(s) => wrkr_value::Value::String(Arc::<str>::from(s.as_str())),
        V::Bytes(b) => wrkr_value::Value::Bytes(bytes::Bytes::copy_from_slice(b.as_ref())),
        V::EnumNumber(n) => wrkr_value::Value::I64(i64::from(*n)),
        V::Message(m) => dynamic_message_to_value(m),
        V::List(list) => wrkr_value::Value::Array(list.iter().map(proto_value_to_value).collect()),
        V::Map(map) => {
            let mut out: HashMap<wrkr_value::MapKey, wrkr_value::Value> = HashMap::new();
            for (k, val) in map.iter() {
                let mk = match k {
                    prost_reflect::MapKey::Bool(b) => wrkr_value::MapKey::Bool(*b),
                    prost_reflect::MapKey::I32(i) => wrkr_value::MapKey::I64(i64::from(*i)),
                    prost_reflect::MapKey::I64(i) => wrkr_value::MapKey::I64(*i),
                    prost_reflect::MapKey::U32(u) => wrkr_value::MapKey::U64(u64::from(*u)),
                    prost_reflect::MapKey::U64(u) => wrkr_value::MapKey::U64(*u),
                    prost_reflect::MapKey::String(s) => {
                        wrkr_value::MapKey::String(Arc::<str>::from(s.as_str()))
                    }
                };
                out.insert(mk, proto_value_to_value(val));
            }
            wrkr_value::Value::Map(out)
        }
    }
}
