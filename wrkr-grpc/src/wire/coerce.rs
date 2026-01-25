pub(super) fn to_bool(value: &wrkr_value::Value) -> bool {
    match value {
        wrkr_value::Value::Bool(b) => *b,
        wrkr_value::Value::String(s) => matches!(s.as_ref(), "true" | "1"),
        wrkr_value::Value::I64(i) => *i != 0,
        wrkr_value::Value::U64(u) => *u != 0,
        _ => false,
    }
}

pub(super) fn map_key_to_i64(key: wrkr_value::MapKey) -> std::result::Result<i64, String> {
    match key {
        wrkr_value::MapKey::I64(i) => Ok(i),
        wrkr_value::MapKey::U64(u) => Ok(u as i64),
        wrkr_value::MapKey::String(s) => s
            .parse::<i64>()
            .map_err(|_| "invalid int64 string".to_string()),
        _ => Err("invalid integer map key".to_string()),
    }
}

pub(super) fn map_key_to_u64(key: wrkr_value::MapKey) -> std::result::Result<u64, String> {
    match key {
        wrkr_value::MapKey::U64(u) => Ok(u),
        wrkr_value::MapKey::I64(i) if i >= 0 => Ok(i as u64),
        wrkr_value::MapKey::String(s) => s
            .parse::<u64>()
            .map_err(|_| "invalid uint64 string".to_string()),
        _ => Err("invalid unsigned integer map key".to_string()),
    }
}

pub(super) fn to_string_lossy(value: &wrkr_value::Value) -> String {
    match value {
        wrkr_value::Value::String(s) => s.to_string(),
        wrkr_value::Value::I64(i) => i.to_string(),
        wrkr_value::Value::U64(u) => u.to_string(),
        wrkr_value::Value::F64(f) => f.to_string(),
        wrkr_value::Value::Bool(b) => b.to_string(),
        _ => String::new(),
    }
}

pub(super) fn to_bytes_lossy(value: &wrkr_value::Value) -> bytes::Bytes {
    match value {
        wrkr_value::Value::Bytes(b) => b.clone(),
        wrkr_value::Value::String(s) => bytes::Bytes::copy_from_slice(s.as_ref().as_bytes()),
        _ => bytes::Bytes::new(),
    }
}

pub(super) fn to_i64(value: &wrkr_value::Value) -> std::result::Result<i64, String> {
    match value {
        wrkr_value::Value::I64(i) => Ok(*i),
        wrkr_value::Value::U64(u) => Ok(*u as i64),
        wrkr_value::Value::F64(f) => Ok(*f as i64),
        wrkr_value::Value::String(s) => s
            .parse::<i64>()
            .map_err(|_| "invalid int64 string".to_string()),
        _ => Err("invalid integer".to_string()),
    }
}

pub(super) fn to_u64(value: &wrkr_value::Value) -> std::result::Result<u64, String> {
    match value {
        wrkr_value::Value::U64(u) => Ok(*u),
        wrkr_value::Value::I64(i) => Ok(*i as u64),
        wrkr_value::Value::F64(f) => Ok(*f as u64),
        wrkr_value::Value::String(s) => s
            .parse::<u64>()
            .map_err(|_| "invalid uint64 string".to_string()),
        _ => Err("invalid integer".to_string()),
    }
}

pub(super) fn to_f64(value: &wrkr_value::Value) -> std::result::Result<f64, String> {
    match value {
        wrkr_value::Value::F64(f) => Ok(*f),
        wrkr_value::Value::I64(i) => Ok(*i as f64),
        wrkr_value::Value::U64(u) => Ok(*u as f64),
        wrkr_value::Value::String(s) => s
            .parse::<f64>()
            .map_err(|_| "invalid float string".to_string()),
        _ => Err("invalid float".to_string()),
    }
}
