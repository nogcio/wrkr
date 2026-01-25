use bytes::{Buf as _, BufMut as _};

use crate::proto::GrpcValueKind;

use super::coerce::{to_bool, to_bytes_lossy, to_f64, to_i64, to_string_lossy, to_u64};
use super::decode::decode_message_for_meta;
use super::primitives::{
    WireType, decode_zigzag64, read_len_delimited, read_variant, write_tag, write_variant,
};
use super::wire_type_for_kind;

/// Decode a single scalar field value (no per-field merging/accumulation).
pub(super) fn decode_scalar_value(
    kind: &GrpcValueKind,
    wire_type: WireType,
    src: &mut bytes::Bytes,
) -> std::result::Result<wrkr_value::Value, String> {
    use GrpcValueKind as K;

    Ok(match kind {
        K::Bool => {
            if wire_type != WireType::Varint {
                return Err("bool field must be varint".to_string());
            }
            wrkr_value::Value::Bool(read_variant(src)? != 0)
        }

        K::String => {
            if wire_type != WireType::Len {
                return Err("string field must be length-delimited".to_string());
            }
            let b = read_len_delimited(src)?;
            match std::str::from_utf8(&b) {
                Ok(s) => wrkr_value::Value::String(s.to_string().into()),
                Err(_) => wrkr_value::Value::String(String::new().into()),
            }
        }

        K::Bytes => {
            if wire_type != WireType::Len {
                return Err("bytes field must be length-delimited".to_string());
            }
            let b = read_len_delimited(src)?;
            wrkr_value::Value::Bytes(b)
        }

        K::Int32 | K::Int64 => {
            if wire_type != WireType::Varint {
                return Err("int field must be varint".to_string());
            }
            wrkr_value::Value::I64(read_variant(src)? as i64)
        }

        K::Uint32 | K::Uint64 => {
            if wire_type != WireType::Varint {
                return Err("uint field must be varint".to_string());
            }
            wrkr_value::Value::U64(read_variant(src)?)
        }

        K::Sint32 | K::Sint64 => {
            if wire_type != WireType::Varint {
                return Err("sint field must be varint".to_string());
            }
            wrkr_value::Value::I64(decode_zigzag64(read_variant(src)?))
        }

        K::Fixed32 => {
            if wire_type != WireType::ThirtyTwoBit {
                return Err("fixed32 field must be 32-bit".to_string());
            }
            if src.remaining() < 4 {
                return Err("unexpected EOF reading fixed32".to_string());
            }
            wrkr_value::Value::U64(u64::from(src.get_u32_le()))
        }

        K::Sfixed32 => {
            if wire_type != WireType::ThirtyTwoBit {
                return Err("sfixed32 field must be 32-bit".to_string());
            }
            if src.remaining() < 4 {
                return Err("unexpected EOF reading sfixed32".to_string());
            }
            wrkr_value::Value::I64(i64::from(src.get_i32_le()))
        }

        K::Float => {
            if wire_type != WireType::ThirtyTwoBit {
                return Err("float field must be 32-bit".to_string());
            }
            if src.remaining() < 4 {
                return Err("unexpected EOF reading float".to_string());
            }
            let bits = src.get_u32_le();
            wrkr_value::Value::F64(f64::from(f32::from_bits(bits)))
        }

        K::Fixed64 => {
            if wire_type != WireType::SixtyFourBit {
                return Err("fixed64 field must be 64-bit".to_string());
            }
            if src.remaining() < 8 {
                return Err("unexpected EOF reading fixed64".to_string());
            }
            wrkr_value::Value::U64(src.get_u64_le())
        }

        K::Sfixed64 => {
            if wire_type != WireType::SixtyFourBit {
                return Err("sfixed64 field must be 64-bit".to_string());
            }
            if src.remaining() < 8 {
                return Err("unexpected EOF reading sfixed64".to_string());
            }
            wrkr_value::Value::I64(src.get_i64_le())
        }

        K::Double => {
            if wire_type != WireType::SixtyFourBit {
                return Err("double field must be 64-bit".to_string());
            }
            if src.remaining() < 8 {
                return Err("unexpected EOF reading double".to_string());
            }
            let bits = src.get_u64_le();
            wrkr_value::Value::F64(f64::from_bits(bits))
        }

        K::Enum(enum_desc) => {
            if wire_type != WireType::Varint {
                return Err("enum field must be varint".to_string());
            }
            let n = read_variant(src)? as i32;
            if let Some(v) = enum_desc.get_value(n) {
                wrkr_value::Value::String(v.name().to_string().into())
            } else {
                wrkr_value::Value::I64(i64::from(n))
            }
        }

        K::Message(meta) => {
            if wire_type != WireType::Len {
                return Err("message field must be length-delimited".to_string());
            }
            let bytes = read_len_delimited(src)?;
            decode_message_for_meta(meta.as_ref(), bytes)?
        }
    })
}

/// Encode a scalar field occurrence (writes tag + value).
pub(super) fn encode_scalar_field(
    field_number: u32,
    kind: &GrpcValueKind,
    value: &wrkr_value::Value,
    out: &mut bytes::BytesMut,
) -> std::result::Result<(), String> {
    use GrpcValueKind as K;

    let wire_type = wire_type_for_kind(kind);
    write_tag(field_number, wire_type, out);

    match kind {
        K::Bool => {
            let b = to_bool(value);
            write_variant(if b { 1 } else { 0 }, out);
        }

        K::String => {
            let s = to_string_lossy(value);
            let b = bytes::Bytes::from(s);
            write_len_delimited(b, out);
        }

        K::Bytes => {
            let b = to_bytes_lossy(value);
            write_len_delimited(b, out);
        }

        K::Int32 | K::Int64 => {
            let n = to_i64(value)?;
            write_variant(n as u64, out);
        }

        K::Uint32 | K::Uint64 => {
            let n = to_u64(value)?;
            write_variant(n, out);
        }

        K::Sint32 | K::Sint64 => {
            let n = to_i64(value)?;
            write_variant(super::primitives::encode_zigzag64(n), out);
        }

        K::Fixed32 => {
            let n = to_u64(value)? as u32;
            out.put_u32_le(n);
        }

        K::Sfixed32 => {
            let n = to_i64(value)? as i32;
            out.put_i32_le(n);
        }

        K::Float => {
            let f = to_f64(value)? as f32;
            out.put_u32_le(f.to_bits());
        }

        K::Fixed64 => {
            let n = to_u64(value)?;
            out.put_u64_le(n);
        }

        K::Sfixed64 => {
            let n = to_i64(value)?;
            out.put_i64_le(n);
        }

        K::Double => {
            let f = to_f64(value)?;
            out.put_u64_le(f.to_bits());
        }

        K::Enum(enum_desc) => {
            // Accept either enum name (string) or numeric.
            let n: i64 = match value {
                wrkr_value::Value::String(s) => enum_desc
                    .get_value_by_name(s.as_ref())
                    .map(|v| i64::from(v.number()))
                    .unwrap_or(0),
                wrkr_value::Value::I64(i) => *i,
                wrkr_value::Value::U64(u) => *u as i64,
                _ => 0,
            };
            write_variant(n as u64, out);
        }

        K::Message(meta) => {
            // Expect object/map; encode using the message meta's fields_by_name.
            let mut buf = bytes::BytesMut::new();
            super::encode::encode_message(meta.fields_by_name(), value, &mut buf)?;
            write_len_delimited(buf.freeze(), out);
        }
    }

    Ok(())
}

fn write_len_delimited(bytes: bytes::Bytes, out: &mut bytes::BytesMut) {
    super::primitives::write_len_delimited(bytes, out);
}
