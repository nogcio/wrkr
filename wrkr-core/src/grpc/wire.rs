use bytes::Buf as _;
use bytes::BufMut as _;

use crate::proto::{GrpcFieldShape, GrpcMethod, GrpcValueKind};

pub(crate) fn encode_value_for_method(
    method: &GrpcMethod,
    value: &wrkr_value::Value,
) -> std::result::Result<bytes::Bytes, String> {
    let mut out = bytes::BytesMut::new();
    encode_message(method.input_fields(), value, &mut out)?;
    Ok(out.freeze())
}

pub(crate) fn decode_value_for_method(
    method: &GrpcMethod,
    bytes: bytes::Bytes,
) -> std::result::Result<wrkr_value::Value, String> {
    decode_message_for_method(method, bytes)
}

fn encode_message(
    fields_by_name: &std::collections::HashMap<
        std::sync::Arc<str>,
        crate::proto::GrpcInputFieldMeta,
    >,
    value: &wrkr_value::Value,
    out: &mut bytes::BytesMut,
) -> std::result::Result<(), String> {
    match value {
        wrkr_value::Value::Object(m) => {
            for (field_name, v) in m {
                let Some(meta) = fields_by_name.get(field_name) else {
                    return Err(format!("unknown field '{field_name}'"));
                };
                encode_field(&meta.shape, &meta.field, v, out)?;
            }
        }
        wrkr_value::Value::Map(m) => {
            for (k, v) in m {
                let wrkr_value::MapKey::String(field_name) = k else {
                    return Err("message expects string field names".to_string());
                };
                let Some(meta) = fields_by_name.get(field_name.as_ref()) else {
                    return Err(format!("unknown field '{field_name}'"));
                };
                encode_field(&meta.shape, &meta.field, v, out)?;
            }
        }
        _ => return Err("message must be an object".to_string()),
    }

    Ok(())
}

fn encode_field(
    shape: &GrpcFieldShape,
    field: &prost_reflect::FieldDescriptor,
    value: &wrkr_value::Value,
    out: &mut bytes::BytesMut,
) -> std::result::Result<(), String> {
    let field_number = field.number();
    if field_number == 0 {
        return Err(format!("invalid field number for '{}'", field.name()));
    }

    match shape {
        GrpcFieldShape::Scalar { kind } => {
            encode_scalar_field(field_number, kind, value, out)?;
        }
        GrpcFieldShape::List { kind } => {
            let wrkr_value::Value::Array(items) = value else {
                return Err(format!("field '{}' must be an array", field.name()));
            };
            for item in items {
                encode_scalar_field(field_number, kind, item, out)?;
            }
        }
        GrpcFieldShape::Map {
            key_kind,
            value_kind,
        } => match value {
            wrkr_value::Value::Object(m) => {
                for (k, v) in m {
                    encode_map_entry(
                        field_number,
                        key_kind,
                        value_kind,
                        wrkr_value::MapKey::String(k.clone()),
                        v,
                        out,
                    )?;
                }
            }
            wrkr_value::Value::Map(m) => {
                for (k, v) in m {
                    encode_map_entry(field_number, key_kind, value_kind, k.clone(), v, out)?;
                }
            }
            _ => return Err(format!("field '{}' must be a map/object", field.name())),
        },
    }

    Ok(())
}

fn encode_map_entry(
    map_field_number: u32,
    key_kind: &prost_reflect::Kind,
    value_kind: &GrpcValueKind,
    key: wrkr_value::MapKey,
    value: &wrkr_value::Value,
    out: &mut bytes::BytesMut,
) -> std::result::Result<(), String> {
    let mut entry = bytes::BytesMut::new();

    // key = 1
    encode_map_key_field(1, key_kind.clone(), key, &mut entry)?;
    // value = 2
    encode_scalar_field(2, value_kind, value, &mut entry)?;

    // outer field: tag + len + entry
    write_tag(map_field_number, WireType::Len, out);
    write_varint(entry.len() as u64, out);
    out.put_slice(&entry);

    Ok(())
}

fn encode_map_key_field(
    field_number: u32,
    kind: prost_reflect::Kind,
    key: wrkr_value::MapKey,
    out: &mut bytes::BytesMut,
) -> std::result::Result<(), String> {
    use prost_reflect::Kind;

    match kind {
        Kind::Bool => {
            let wrkr_value::MapKey::Bool(b) = key else {
                return Err("map key must be bool".to_string());
            };
            write_tag(field_number, WireType::Varint, out);
            write_varint(u64::from(b), out);
        }
        Kind::String => {
            let wrkr_value::MapKey::String(s) = key else {
                return Err("map key must be string".to_string());
            };
            write_tag(field_number, WireType::Len, out);
            write_len_delimited(s.as_bytes(), out);
        }
        Kind::Int32 | Kind::Sint32 | Kind::Sfixed32 => {
            let n = map_key_to_i64(key)?;
            write_tag(field_number, WireType::Varint, out);
            match kind {
                Kind::Sint32 => write_varint(encode_zigzag64(n) as u64, out),
                _ => write_varint(n as u64, out),
            }
        }
        Kind::Int64 | Kind::Sint64 | Kind::Sfixed64 => {
            let n = map_key_to_i64(key)?;
            write_tag(field_number, WireType::Varint, out);
            match kind {
                Kind::Sint64 => write_varint(encode_zigzag64(n), out),
                _ => write_varint(n as u64, out),
            }
        }
        Kind::Uint32 | Kind::Fixed32 => {
            let n = map_key_to_u64(key)?;
            write_tag(field_number, WireType::Varint, out);
            write_varint(n as u64, out);
        }
        Kind::Uint64 | Kind::Fixed64 => {
            let n = map_key_to_u64(key)?;
            write_tag(field_number, WireType::Varint, out);
            write_varint(n, out);
        }
        other => Err(format!("unsupported map key kind: {other:?}"))?,
    }

    Ok(())
}

fn encode_scalar_field(
    field_number: u32,
    kind: &GrpcValueKind,
    value: &wrkr_value::Value,
    out: &mut bytes::BytesMut,
) -> std::result::Result<(), String> {
    use GrpcValueKind as K;

    match kind {
        K::Bool => {
            write_tag(field_number, WireType::Varint, out);
            let b = to_bool(value);
            write_varint(u64::from(b), out);
        }
        K::String => {
            write_tag(field_number, WireType::Len, out);
            let s = to_string_lossy(value);
            write_len_delimited(s.as_bytes(), out);
        }
        K::Bytes => {
            write_tag(field_number, WireType::Len, out);
            let b = to_bytes_lossy(value);
            write_len_delimited(&b, out);
        }

        K::Int32 | K::Sfixed32 => {
            write_tag(field_number, WireType::Varint, out);
            write_varint(to_i64(value)? as u64, out);
        }
        K::Sint32 => {
            write_tag(field_number, WireType::Varint, out);
            write_varint(encode_zigzag64(to_i64(value)?) as u64, out);
        }
        K::Int64 | K::Sfixed64 => {
            write_tag(field_number, WireType::Varint, out);
            write_varint(to_i64(value)? as u64, out);
        }
        K::Sint64 => {
            write_tag(field_number, WireType::Varint, out);
            write_varint(encode_zigzag64(to_i64(value)?), out);
        }

        K::Uint32 | K::Uint64 => {
            write_tag(field_number, WireType::Varint, out);
            write_varint(to_u64(value)?, out);
        }

        K::Fixed32 => {
            write_tag(field_number, WireType::Bit32, out);
            out.put_u32_le(to_u64(value)? as u32);
        }
        K::Fixed64 => {
            write_tag(field_number, WireType::Bit64, out);
            out.put_u64_le(to_u64(value)?);
        }

        K::Float => {
            write_tag(field_number, WireType::Bit32, out);
            out.put_f32_le(to_f64(value)? as f32);
        }
        K::Double => {
            write_tag(field_number, WireType::Bit64, out);
            out.put_f64_le(to_f64(value)?);
        }

        K::Enum(_enum_desc) => {
            // We accept either string name (not implemented here) or numeric.
            // Keep behavior aligned with existing conversion: numeric is fine.
            write_tag(field_number, WireType::Varint, out);
            write_varint(to_i64(value)? as u64, out);
        }

        K::Message(meta) => {
            write_tag(field_number, WireType::Len, out);
            let mut buf = bytes::BytesMut::new();
            encode_message(meta.fields_by_name(), value, &mut buf)?;
            write_varint(buf.len() as u64, out);
            out.put_slice(&buf);
        }
    }

    Ok(())
}

fn decode_message_for_method(
    method: &GrpcMethod,
    bytes: bytes::Bytes,
) -> std::result::Result<wrkr_value::Value, String> {
    let fields = method.output_fields();
    let by_number = method.output_field_index_by_number();

    let mut src = bytes;
    let mut out: wrkr_value::ObjectMap = wrkr_value::ObjectMap::with_capacity(fields.len());

    while src.has_remaining() {
        let tag = read_varint(&mut src)?;
        if tag == 0 {
            return Err("invalid protobuf tag 0".to_string());
        }

        let field_number = (tag >> 3) as u32;
        let wire_type = WireType::try_from((tag & 0x7) as u8)?;

        let Some(&idx) = by_number.get(&field_number) else {
            skip_value(wire_type, &mut src)?;
            continue;
        };

        let meta = &fields[idx];

        if let GrpcFieldShape::Map {
            key_kind,
            value_kind,
        } = &meta.shape
        {
            decode_map_entry_into_object(
                &mut out, &meta.name, key_kind, value_kind, wire_type, &mut src,
            )?;
        } else {
            let v = decode_field_value(&meta.shape, wire_type, &mut src)?;
            merge_decoded_field(&mut out, &meta.name, &meta.shape, v)?;
        }
    }

    Ok(wrkr_value::Value::Object(out))
}

fn decode_message_for_meta(
    meta: &crate::proto::GrpcMessageMeta,
    bytes: bytes::Bytes,
) -> std::result::Result<wrkr_value::Value, String> {
    let by_number = meta.fields_by_number();
    let mut src = bytes;
    let mut out: wrkr_value::ObjectMap = wrkr_value::ObjectMap::with_capacity(by_number.len());

    while src.has_remaining() {
        let tag = read_varint(&mut src)?;
        if tag == 0 {
            return Err("invalid protobuf tag 0".to_string());
        }

        let field_number = (tag >> 3) as u32;
        let wire_type = WireType::try_from((tag & 0x7) as u8)?;

        let Some((name, shape)) = by_number.get(&field_number) else {
            skip_value(wire_type, &mut src)?;
            continue;
        };

        if let GrpcFieldShape::Map {
            key_kind,
            value_kind,
        } = shape
        {
            decode_map_entry_into_object(
                &mut out, name, key_kind, value_kind, wire_type, &mut src,
            )?;
        } else {
            let v = decode_field_value(shape, wire_type, &mut src)?;
            merge_decoded_field(&mut out, name, shape, v)?;
        }
    }

    Ok(wrkr_value::Value::Object(out))
}

fn merge_decoded_field(
    out: &mut wrkr_value::ObjectMap,
    name: &std::sync::Arc<str>,
    shape: &GrpcFieldShape,
    v: wrkr_value::Value,
) -> std::result::Result<(), String> {
    match shape {
        GrpcFieldShape::Scalar { .. } => {
            out.insert(name.clone(), v);
            Ok(())
        }
        GrpcFieldShape::List { .. } => {
            match out.get_mut(name) {
                None => {
                    out.insert(name.clone(), wrkr_value::Value::Array(vec![v]));
                }
                Some(wrkr_value::Value::Array(items)) => {
                    items.push(v);
                }
                Some(existing) => {
                    let prev = std::mem::replace(existing, wrkr_value::Value::Null);
                    *existing = wrkr_value::Value::Array(vec![prev, v]);
                }
            }
            Ok(())
        }
        GrpcFieldShape::Map { .. } => {
            let wrkr_value::Value::Map(mut new_map) = v else {
                return Err("decoded map field was not a map".to_string());
            };

            match out.get_mut(name) {
                None => {
                    out.insert(name.clone(), wrkr_value::Value::Map(new_map));
                }
                Some(wrkr_value::Value::Map(existing)) => {
                    existing.reserve(new_map.len());
                    existing.extend(new_map.drain());
                }
                Some(existing) => {
                    *existing = wrkr_value::Value::Map(new_map);
                }
            }
            Ok(())
        }
    }
}

fn decode_field_value(
    shape: &GrpcFieldShape,
    wire_type: WireType,
    src: &mut bytes::Bytes,
) -> std::result::Result<wrkr_value::Value, String> {
    match shape {
        GrpcFieldShape::Scalar { kind } => decode_scalar_value(kind, wire_type, src),
        GrpcFieldShape::List { kind } => {
            // Minimal list support: decode one element (caller may overwrite).
            // Not needed for analytics AggregateResult.
            decode_scalar_value(kind, wire_type, src)
        }
        GrpcFieldShape::Map {
            key_kind,
            value_kind,
        } => {
            if wire_type != WireType::Len {
                return Err("map field must be length-delimited".to_string());
            }
            let bytes = read_len_delimited(src)?;
            let (k, v) = decode_map_entry(key_kind, value_kind, bytes)?;
            let mut map: wrkr_value::MapMap = wrkr_value::MapMap::with_capacity(1);
            map.insert(k, v);
            Ok(wrkr_value::Value::Map(map))
        }
    }
}

fn decode_map_entry(
    key_kind: &prost_reflect::Kind,
    value_kind: &GrpcValueKind,
    mut bytes: bytes::Bytes,
) -> std::result::Result<(wrkr_value::MapKey, wrkr_value::Value), String> {
    let mut key: Option<wrkr_value::MapKey> = None;
    let mut value: Option<wrkr_value::Value> = None;

    while bytes.has_remaining() {
        let tag = read_varint(&mut bytes)?;
        if tag == 0 {
            return Err("invalid protobuf tag 0".to_string());
        }
        let field_number = (tag >> 3) as u32;
        let wire_type = WireType::try_from((tag & 0x7) as u8)?;

        match field_number {
            1 => {
                key = Some(decode_map_key_value(
                    key_kind.clone(),
                    wire_type,
                    &mut bytes,
                )?);
            }
            2 => {
                value = Some(decode_scalar_value(value_kind, wire_type, &mut bytes)?);
            }
            _ => skip_value(wire_type, &mut bytes)?,
        }
    }

    let key = key.ok_or_else(|| "missing map entry key".to_string())?;
    let value = value.ok_or_else(|| "missing map entry value".to_string())?;
    Ok((key, value))
}

fn decode_map_entry_into_object(
    out: &mut wrkr_value::ObjectMap,
    name: &std::sync::Arc<str>,
    key_kind: &prost_reflect::Kind,
    value_kind: &GrpcValueKind,
    wire_type: WireType,
    src: &mut bytes::Bytes,
) -> std::result::Result<(), String> {
    if wire_type != WireType::Len {
        return Err("map field must be length-delimited".to_string());
    }

    let bytes = read_len_delimited(src)?;
    let (k, v) = decode_map_entry(key_kind, value_kind, bytes)?;

    match out.get_mut(name) {
        None => {
            let mut map: wrkr_value::MapMap = wrkr_value::MapMap::with_capacity(1);
            map.insert(k, v);
            out.insert(name.clone(), wrkr_value::Value::Map(map));
            Ok(())
        }
        Some(wrkr_value::Value::Map(existing)) => {
            existing.insert(k, v);
            Ok(())
        }
        Some(existing) => {
            let mut map: wrkr_value::MapMap = wrkr_value::MapMap::with_capacity(1);
            map.insert(k, v);
            *existing = wrkr_value::Value::Map(map);
            Ok(())
        }
    }
}

fn decode_map_key_value(
    kind: prost_reflect::Kind,
    wire_type: WireType,
    src: &mut bytes::Bytes,
) -> std::result::Result<wrkr_value::MapKey, String> {
    use prost_reflect::Kind;

    match kind {
        Kind::String => {
            if wire_type != WireType::Len {
                return Err("string map key must be length-delimited".to_string());
            }
            let b = read_len_delimited(src)?;
            let s = std::str::from_utf8(b.as_ref()).map_err(|_| "invalid utf8".to_string())?;
            Ok(wrkr_value::MapKey::String(std::sync::Arc::<str>::from(s)))
        }
        Kind::Bool => {
            if wire_type != WireType::Varint {
                return Err("bool map key must be varint".to_string());
            }
            Ok(wrkr_value::MapKey::Bool(read_varint(src)? != 0))
        }
        Kind::Int32
        | Kind::Sint32
        | Kind::Sfixed32
        | Kind::Int64
        | Kind::Sint64
        | Kind::Sfixed64 => {
            if wire_type != WireType::Varint {
                return Err("int map key must be varint".to_string());
            }
            let n = read_varint(src)? as i64;
            Ok(wrkr_value::MapKey::I64(n))
        }
        Kind::Uint32 | Kind::Fixed32 | Kind::Uint64 | Kind::Fixed64 => {
            if wire_type != WireType::Varint {
                return Err("uint map key must be varint".to_string());
            }
            Ok(wrkr_value::MapKey::U64(read_varint(src)?))
        }
        other => Err(format!("unsupported map key kind: {other:?}"))?,
    }
}

fn decode_scalar_value(
    kind: &GrpcValueKind,
    wire_type: WireType,
    src: &mut bytes::Bytes,
) -> std::result::Result<wrkr_value::Value, String> {
    use GrpcValueKind as K;

    match kind {
        K::Bool => {
            if wire_type != WireType::Varint {
                return Err("bool must be varint".to_string());
            }
            Ok(wrkr_value::Value::Bool(read_varint(src)? != 0))
        }
        K::String => {
            if wire_type != WireType::Len {
                return Err("string must be len".to_string());
            }
            let b = read_len_delimited(src)?;
            let s = std::str::from_utf8(b.as_ref()).map_err(|_| "invalid utf8".to_string())?;
            Ok(wrkr_value::Value::String(std::sync::Arc::<str>::from(s)))
        }
        K::Bytes => {
            if wire_type != WireType::Len {
                return Err("bytes must be len".to_string());
            }
            let b = read_len_delimited(src)?;
            Ok(wrkr_value::Value::Bytes(b))
        }

        K::Int32 | K::Sfixed32 | K::Int64 | K::Sfixed64 | K::Enum(_) => {
            if wire_type != WireType::Varint {
                return Err("int/enum must be varint".to_string());
            }
            Ok(wrkr_value::Value::I64(read_varint(src)? as i64))
        }
        K::Sint32 | K::Sint64 => {
            if wire_type != WireType::Varint {
                return Err("sint must be varint".to_string());
            }
            Ok(wrkr_value::Value::I64(decode_zigzag64(read_varint(src)?)))
        }
        K::Uint32 | K::Uint64 => {
            if wire_type != WireType::Varint {
                return Err("uint must be varint".to_string());
            }
            Ok(wrkr_value::Value::U64(read_varint(src)?))
        }

        K::Fixed32 | K::Float => {
            if wire_type != WireType::Bit32 {
                return Err("32-bit must be bit32".to_string());
            }
            let v = src.get_u32_le();
            match kind {
                K::Fixed32 => Ok(wrkr_value::Value::U64(u64::from(v))),
                K::Float => Ok(wrkr_value::Value::F64(f64::from(f32::from_bits(v)))),
                _ => unreachable!(),
            }
        }
        K::Fixed64 | K::Double => {
            if wire_type != WireType::Bit64 {
                return Err("64-bit must be bit64".to_string());
            }
            let v = src.get_u64_le();
            match kind {
                K::Fixed64 => Ok(wrkr_value::Value::U64(v)),
                K::Double => Ok(wrkr_value::Value::F64(f64::from_bits(v))),
                _ => unreachable!(),
            }
        }

        K::Message(meta) => {
            if wire_type != WireType::Len {
                return Err("message must be len".to_string());
            }
            let b = read_len_delimited(src)?;
            decode_message_for_meta(meta.as_ref(), b)
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum WireType {
    Varint = 0,
    Bit64 = 1,
    Len = 2,
    Bit32 = 5,
}

impl TryFrom<u8> for WireType {
    type Error = String;

    fn try_from(value: u8) -> std::result::Result<Self, Self::Error> {
        Ok(match value {
            0 => Self::Varint,
            1 => Self::Bit64,
            2 => Self::Len,
            5 => Self::Bit32,
            other => return Err(format!("unsupported wire type {other}")),
        })
    }
}

fn write_tag(field_number: u32, wire_type: WireType, out: &mut bytes::BytesMut) {
    let tag = (u64::from(field_number) << 3) | (wire_type as u64);
    write_varint(tag, out);
}

fn write_len_delimited(bytes: &[u8], out: &mut bytes::BytesMut) {
    write_varint(bytes.len() as u64, out);
    out.put_slice(bytes);
}

fn write_varint(mut v: u64, out: &mut bytes::BytesMut) {
    while v >= 0x80 {
        out.put_u8(((v as u8) & 0x7F) | 0x80);
        v >>= 7;
    }
    out.put_u8(v as u8);
}

fn read_varint(src: &mut bytes::Bytes) -> std::result::Result<u64, String> {
    let mut shift = 0;
    let mut out: u64 = 0;

    for _ in 0..10 {
        if !src.has_remaining() {
            return Err("unexpected EOF reading varint".to_string());
        }
        let b = src.get_u8();
        out |= u64::from(b & 0x7F) << shift;
        if (b & 0x80) == 0 {
            return Ok(out);
        }
        shift += 7;
    }

    Err("varint too long".to_string())
}

fn read_len_delimited(src: &mut bytes::Bytes) -> std::result::Result<bytes::Bytes, String> {
    let len = read_varint(src)? as usize;
    if src.remaining() < len {
        return Err("unexpected EOF reading len-delimited".to_string());
    }
    Ok(src.copy_to_bytes(len))
}

fn skip_value(wire_type: WireType, src: &mut bytes::Bytes) -> std::result::Result<(), String> {
    match wire_type {
        WireType::Varint => {
            let _ = read_varint(src)?;
        }
        WireType::Bit64 => {
            if src.remaining() < 8 {
                return Err("unexpected EOF skipping 64-bit".to_string());
            }
            src.advance(8);
        }
        WireType::Len => {
            let len = read_varint(src)? as usize;
            if src.remaining() < len {
                return Err("unexpected EOF skipping len".to_string());
            }
            src.advance(len);
        }
        WireType::Bit32 => {
            if src.remaining() < 4 {
                return Err("unexpected EOF skipping 32-bit".to_string());
            }
            src.advance(4);
        }
    }

    Ok(())
}

fn encode_zigzag64(v: i64) -> u64 {
    ((v << 1) ^ (v >> 63)) as u64
}

fn decode_zigzag64(v: u64) -> i64 {
    ((v >> 1) as i64) ^ (-((v & 1) as i64))
}

fn to_bool(value: &wrkr_value::Value) -> bool {
    match value {
        wrkr_value::Value::Bool(b) => *b,
        wrkr_value::Value::String(s) => matches!(s.as_ref(), "true" | "1"),
        wrkr_value::Value::I64(i) => *i != 0,
        wrkr_value::Value::U64(u) => *u != 0,
        _ => false,
    }
}

fn to_string_lossy(value: &wrkr_value::Value) -> String {
    match value {
        wrkr_value::Value::String(s) => s.to_string(),
        wrkr_value::Value::I64(i) => i.to_string(),
        wrkr_value::Value::U64(u) => u.to_string(),
        wrkr_value::Value::F64(f) => f.to_string(),
        wrkr_value::Value::Bool(b) => b.to_string(),
        _ => String::new(),
    }
}

fn to_bytes_lossy(value: &wrkr_value::Value) -> bytes::Bytes {
    match value {
        wrkr_value::Value::Bytes(b) => b.clone(),
        wrkr_value::Value::String(s) => bytes::Bytes::copy_from_slice(s.as_ref().as_bytes()),
        _ => bytes::Bytes::new(),
    }
}

fn to_i64(value: &wrkr_value::Value) -> std::result::Result<i64, String> {
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

fn to_u64(value: &wrkr_value::Value) -> std::result::Result<u64, String> {
    match value {
        wrkr_value::Value::U64(u) => Ok(*u),
        wrkr_value::Value::I64(i) if *i >= 0 => Ok(*i as u64),
        wrkr_value::Value::F64(f) if *f >= 0.0 => Ok(*f as u64),
        wrkr_value::Value::String(s) => s
            .parse::<u64>()
            .map_err(|_| "invalid uint64 string".to_string()),
        _ => Err("invalid unsigned integer".to_string()),
    }
}

fn to_f64(value: &wrkr_value::Value) -> std::result::Result<f64, String> {
    match value {
        wrkr_value::Value::F64(f) => Ok(*f),
        wrkr_value::Value::I64(i) => Ok(*i as f64),
        wrkr_value::Value::U64(u) => Ok(*u as f64),
        wrkr_value::Value::String(s) => s
            .parse::<f64>()
            .map_err(|_| "invalid float string".to_string()),
        _ => Err("invalid number".to_string()),
    }
}

fn map_key_to_i64(key: wrkr_value::MapKey) -> std::result::Result<i64, String> {
    match key {
        wrkr_value::MapKey::I64(i) => Ok(i),
        wrkr_value::MapKey::U64(u) => Ok(u as i64),
        wrkr_value::MapKey::String(s) => s
            .parse::<i64>()
            .map_err(|_| "invalid int64 string".to_string()),
        _ => Err("invalid integer map key".to_string()),
    }
}

fn map_key_to_u64(key: wrkr_value::MapKey) -> std::result::Result<u64, String> {
    match key {
        wrkr_value::MapKey::U64(u) => Ok(u),
        wrkr_value::MapKey::I64(i) if i >= 0 => Ok(i as u64),
        wrkr_value::MapKey::String(s) => s
            .parse::<u64>()
            .map_err(|_| "invalid uint64 string".to_string()),
        _ => Err("invalid unsigned integer map key".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zigzag_roundtrip() {
        for &v in &[-123_i64, -1, 0, 1, 123, i64::MIN / 2, i64::MAX / 2] {
            let enc = encode_zigzag64(v);
            let dec = decode_zigzag64(enc);
            assert_eq!(dec, v);
        }
    }

    #[test]
    fn merge_decoded_field_merges_map_entries() {
        let mut out: wrkr_value::ObjectMap = wrkr_value::ObjectMap::new();
        let name = std::sync::Arc::<str>::from("amount_by_country");

        let shape = GrpcFieldShape::Map {
            key_kind: prost_reflect::Kind::String,
            value_kind: GrpcValueKind::Int64,
        };

        let mut m1: wrkr_value::MapMap = wrkr_value::MapMap::new();
        m1.insert(
            wrkr_value::MapKey::String(std::sync::Arc::<str>::from("US")),
            wrkr_value::Value::I64(10),
        );
        assert!(merge_decoded_field(&mut out, &name, &shape, wrkr_value::Value::Map(m1)).is_ok());

        let mut m2: wrkr_value::MapMap = wrkr_value::MapMap::new();
        m2.insert(
            wrkr_value::MapKey::String(std::sync::Arc::<str>::from("FR")),
            wrkr_value::Value::I64(20),
        );
        assert!(merge_decoded_field(&mut out, &name, &shape, wrkr_value::Value::Map(m2)).is_ok());

        let Some(wrkr_value::Value::Map(got)) = out.get(&name) else {
            panic!("expected a map value");
        };
        assert_eq!(got.len(), 2);
    }
}
