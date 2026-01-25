use bytes::{Buf as _, BufMut as _};

use crate::proto::GrpcValueKind;

use super::coerce::{map_key_to_i64, map_key_to_u64};
use super::primitives::{
    WireType, encode_zigzag64, read_len_delimited, read_variant, write_len_delimited, write_tag,
    write_variant,
};
use super::scalar::{decode_scalar_value, encode_scalar_field};

pub(super) fn decode_map_entry_into_object(
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

pub(super) fn decode_map_entry(
    key_kind: &prost_reflect::Kind,
    value_kind: &GrpcValueKind,
    mut bytes: bytes::Bytes,
) -> std::result::Result<(wrkr_value::MapKey, wrkr_value::Value), String> {
    let mut key: Option<wrkr_value::MapKey> = None;
    let mut value: Option<wrkr_value::Value> = None;

    while bytes.has_remaining() {
        let tag = read_variant(&mut bytes)?;
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
            _ => super::primitives::skip_value(wire_type, &mut bytes)?,
        }
    }

    let key = key.ok_or_else(|| "missing map entry key".to_string())?;
    let value = value.ok_or_else(|| "missing map entry value".to_string())?;
    Ok((key, value))
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
            if wire_type != WireType::Variant {
                return Err("bool map key must be varint".to_string());
            }
            Ok(wrkr_value::MapKey::Bool(read_variant(src)? != 0))
        }
        Kind::Int32
        | Kind::Sint32
        | Kind::Sfixed32
        | Kind::Int64
        | Kind::Sint64
        | Kind::Sfixed64 => {
            if wire_type != WireType::Variant {
                return Err("int map key must be varint".to_string());
            }
            let n = read_variant(src)? as i64;
            Ok(wrkr_value::MapKey::I64(n))
        }
        Kind::Uint32 | Kind::Fixed32 | Kind::Uint64 | Kind::Fixed64 => {
            if wire_type != WireType::Variant {
                return Err("uint map key must be varint".to_string());
            }
            Ok(wrkr_value::MapKey::U64(read_variant(src)?))
        }
        other => Err(format!("unsupported map key kind: {other:?}"))?,
    }
}

pub(super) fn encode_map_entry(
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
    write_variant(entry.len() as u64, out);
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
            write_tag(field_number, WireType::Variant, out);
            write_variant(u64::from(b), out);
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
            write_tag(field_number, WireType::Variant, out);
            match kind {
                Kind::Sint32 => write_variant(encode_zigzag64(n) as u64, out),
                _ => write_variant(n as u64, out),
            }
        }
        Kind::Int64 | Kind::Sint64 | Kind::Sfixed64 => {
            let n = map_key_to_i64(key)?;
            write_tag(field_number, WireType::Variant, out);
            match kind {
                Kind::Sint64 => write_variant(encode_zigzag64(n), out),
                _ => write_variant(n as u64, out),
            }
        }
        Kind::Uint32 | Kind::Fixed32 => {
            let n = map_key_to_u64(key)?;
            write_tag(field_number, WireType::Variant, out);
            write_variant(n as u64, out);
        }
        Kind::Uint64 | Kind::Fixed64 => {
            let n = map_key_to_u64(key)?;
            write_tag(field_number, WireType::Variant, out);
            write_variant(n, out);
        }
        other => Err(format!("unsupported map key kind: {other:?}"))?,
    }

    Ok(())
}
