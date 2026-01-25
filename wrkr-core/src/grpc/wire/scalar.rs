use bytes::{Buf as _, BufMut as _};

use crate::proto::GrpcValueKind;

use super::coerce::{to_bool, to_bytes_lossy, to_f64, to_i64, to_string_lossy, to_u64};
use super::primitives::{
    WireType, decode_zigzag64, encode_zigzag64, read_len_delimited, read_variant,
    write_len_delimited, write_tag, write_variant,
};

pub(super) fn decode_scalar_value(
    kind: &GrpcValueKind,
    wire_type: WireType,
    src: &mut bytes::Bytes,
) -> std::result::Result<wrkr_value::Value, String> {
    use GrpcValueKind as K;

    match kind {
        K::Bool => {
            if wire_type != WireType::Variant {
                return Err("bool must be varint".to_string());
            }
            Ok(wrkr_value::Value::Bool(read_variant(src)? != 0))
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

        K::Int32 | K::Int64 | K::Enum(_) => {
            if wire_type != WireType::Variant {
                return Err("int/enum must be varint".to_string());
            }
            Ok(wrkr_value::Value::I64(read_variant(src)? as i64))
        }
        K::Sint32 | K::Sint64 => {
            if wire_type != WireType::Variant {
                return Err("sint must be varint".to_string());
            }
            Ok(wrkr_value::Value::I64(decode_zigzag64(read_variant(src)?)))
        }
        K::Uint32 | K::Uint64 => {
            if wire_type != WireType::Variant {
                return Err("uint must be varint".to_string());
            }
            Ok(wrkr_value::Value::U64(read_variant(src)?))
        }

        K::Fixed32 | K::Sfixed32 | K::Float => {
            if wire_type != WireType::Bit32 {
                return Err("32-bit must be bit32".to_string());
            }
            match kind {
                K::Fixed32 => Ok(wrkr_value::Value::U64(u64::from(src.get_u32_le()))),
                K::Sfixed32 => Ok(wrkr_value::Value::I64(i64::from(src.get_i32_le()))),
                K::Float => {
                    let v = src.get_u32_le();
                    Ok(wrkr_value::Value::F64(f64::from(f32::from_bits(v))))
                }
                _ => unreachable!(),
            }
        }
        K::Fixed64 | K::Sfixed64 | K::Double => {
            if wire_type != WireType::Bit64 {
                return Err("64-bit must be bit64".to_string());
            }
            match kind {
                K::Fixed64 => Ok(wrkr_value::Value::U64(src.get_u64_le())),
                K::Sfixed64 => Ok(wrkr_value::Value::I64(src.get_i64_le())),
                K::Double => Ok(wrkr_value::Value::F64(f64::from_bits(src.get_u64_le()))),
                _ => unreachable!(),
            }
        }

        K::Message(meta) => {
            if wire_type != WireType::Len {
                return Err("message must be len".to_string());
            }
            let b = read_len_delimited(src)?;
            super::decode::decode_message_for_meta(meta.as_ref(), b)
        }
    }
}

pub(super) fn encode_scalar_field(
    field_number: u32,
    kind: &GrpcValueKind,
    value: &wrkr_value::Value,
    out: &mut bytes::BytesMut,
) -> std::result::Result<(), String> {
    use GrpcValueKind as K;

    match kind {
        K::Bool => {
            write_tag(field_number, WireType::Variant, out);
            let b = to_bool(value);
            write_variant(u64::from(b), out);
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

        K::Int32 => {
            write_tag(field_number, WireType::Variant, out);
            write_variant(to_i64(value)? as u64, out);
        }
        K::Sint32 => {
            write_tag(field_number, WireType::Variant, out);
            write_variant(encode_zigzag64(to_i64(value)?) as u64, out);
        }
        K::Int64 => {
            write_tag(field_number, WireType::Variant, out);
            write_variant(to_i64(value)? as u64, out);
        }
        K::Sint64 => {
            write_tag(field_number, WireType::Variant, out);
            write_variant(encode_zigzag64(to_i64(value)?), out);
        }

        K::Sfixed32 => {
            write_tag(field_number, WireType::Bit32, out);
            out.put_i32_le(to_i64(value)? as i32);
        }
        K::Sfixed64 => {
            write_tag(field_number, WireType::Bit64, out);
            out.put_i64_le(to_i64(value)?);
        }

        K::Uint32 | K::Uint64 => {
            write_tag(field_number, WireType::Variant, out);
            write_variant(to_u64(value)?, out);
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

        K::Enum(enum_desc) => {
            // Accept enum by name ("FOO") or numeric.
            write_tag(field_number, WireType::Variant, out);

            let n = match value {
                wrkr_value::Value::String(s) => {
                    if let Some(v) = enum_desc.get_value_by_name(s.as_ref()) {
                        i64::from(v.number())
                    } else {
                        to_i64(value)?
                    }
                }
                _ => to_i64(value)?,
            };

            write_variant(n as u64, out);
        }

        K::Message(meta) => {
            write_tag(field_number, WireType::Len, out);
            let mut buf = bytes::BytesMut::new();
            super::encode::encode_message(meta.fields_by_name(), value, &mut buf)?;
            write_variant(buf.len() as u64, out);
            out.put_slice(&buf);
        }
    }

    Ok(())
}
