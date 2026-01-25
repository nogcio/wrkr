use bytes::Buf as _;

use crate::proto::GrpcValueKind;

use super::coerce::{map_key_to_i64, map_key_to_u64};
use super::primitives::{
    WireType, read_len_delimited, read_variant, write_len_delimited, write_tag, write_variant,
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
                key = Some(decode_map_key(key_kind, wire_type, &mut bytes)?);
            }
            2 => {
                value = Some(decode_scalar_value(value_kind, wire_type, &mut bytes)?);
            }
            _ => {
                super::primitives::skip_value(wire_type, &mut bytes)?;
            }
        }
    }

    let k = key.ok_or_else(|| "map entry missing key".to_string())?;
    let v = value.ok_or_else(|| "map entry missing value".to_string())?;

    Ok((k, v))
}

fn decode_map_key(
    kind: &prost_reflect::Kind,
    wire_type: WireType,
    src: &mut bytes::Bytes,
) -> std::result::Result<wrkr_value::MapKey, String> {
    use prost_reflect::Kind as K;

    Ok(match kind {
        K::String => {
            if wire_type != WireType::Len {
                return Err("map key (string) must be length-delimited".to_string());
            }
            let b = read_len_delimited(src)?;
            let s = std::str::from_utf8(&b)
                .map_err(|_| "invalid utf-8 map key".to_string())?
                .to_string();
            wrkr_value::MapKey::String(s.into())
        }
        K::Bool => {
            if wire_type != WireType::Varint {
                return Err("map key (bool) must be varint".to_string());
            }
            wrkr_value::MapKey::Bool(read_variant(src)? != 0)
        }

        K::Int32 | K::Sint32 | K::Sfixed32 => {
            if wire_type != WireType::Varint {
                return Err("map key (int32) must be varint".to_string());
            }
            wrkr_value::MapKey::I64(read_variant(src)? as i64)
        }
        K::Int64 | K::Sint64 | K::Sfixed64 => {
            if wire_type != WireType::Varint {
                return Err("map key (int64) must be varint".to_string());
            }
            wrkr_value::MapKey::I64(read_variant(src)? as i64)
        }

        K::Uint32 | K::Fixed32 => {
            if wire_type != WireType::Varint {
                return Err("map key (uint32) must be varint".to_string());
            }
            wrkr_value::MapKey::U64(read_variant(src)?)
        }
        K::Uint64 | K::Fixed64 => {
            if wire_type != WireType::Varint {
                return Err("map key (uint64) must be varint".to_string());
            }
            wrkr_value::MapKey::U64(read_variant(src)?)
        }

        other => {
            return Err(format!("unsupported map key type for proto kind {other:?}"));
        }
    })
}

pub(super) fn encode_map_entry(
    field_number: u32,
    key_kind: &prost_reflect::Kind,
    value_kind: &GrpcValueKind,
    key: wrkr_value::MapKey,
    value: &wrkr_value::Value,
    out: &mut bytes::BytesMut,
) -> std::result::Result<(), String> {
    // Map entries are encoded as a length-delimited embedded message with fields:
    // 1: key, 2: value.
    let mut entry = bytes::BytesMut::new();

    encode_map_key(key_kind, key, &mut entry)?;
    encode_scalar_field(2, value_kind, value, &mut entry)?;

    write_tag(field_number, WireType::Len, out);
    write_len_delimited(entry.freeze(), out);

    Ok(())
}

fn encode_map_key(
    kind: &prost_reflect::Kind,
    key: wrkr_value::MapKey,
    out: &mut bytes::BytesMut,
) -> std::result::Result<(), String> {
    use prost_reflect::Kind as K;

    match kind {
        K::String => {
            let wrkr_value::MapKey::String(s) = key else {
                return Err("map key must be a string".to_string());
            };
            write_tag(1, WireType::Len, out);
            write_len_delimited(bytes::Bytes::copy_from_slice(s.as_ref().as_bytes()), out);
            Ok(())
        }
        K::Bool => {
            let wrkr_value::MapKey::Bool(b) = key else {
                return Err("map key must be boolean".to_string());
            };
            write_tag(1, WireType::Varint, out);
            write_variant(if b { 1 } else { 0 }, out);
            Ok(())
        }
        K::Int32 | K::Sint32 | K::Sfixed32 => {
            let n = map_key_to_i64(key)?;
            write_tag(1, WireType::Varint, out);
            write_variant(n as u64, out);
            Ok(())
        }
        K::Int64 | K::Sint64 | K::Sfixed64 => {
            let n = map_key_to_i64(key)?;
            write_tag(1, WireType::Varint, out);
            write_variant(n as u64, out);
            Ok(())
        }
        K::Uint32 | K::Fixed32 => {
            let n = map_key_to_u64(key)?;
            write_tag(1, WireType::Varint, out);
            write_variant(n as u64, out);
            Ok(())
        }
        K::Uint64 | K::Fixed64 => {
            let n = map_key_to_u64(key)?;
            write_tag(1, WireType::Varint, out);
            write_variant(n, out);
            Ok(())
        }
        other => Err(format!("unsupported map key type for proto kind {other:?}")),
    }
}
