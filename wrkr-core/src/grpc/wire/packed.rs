use bytes::Buf as _;

use crate::proto::GrpcValueKind;

use super::primitives::{decode_zigzag64, read_variant};

pub(super) fn kind_is_packable(kind: &GrpcValueKind) -> bool {
    use GrpcValueKind as K;

    matches!(
        kind,
        K::Bool
            | K::Int32
            | K::Sint32
            | K::Sfixed32
            | K::Int64
            | K::Sint64
            | K::Sfixed64
            | K::Uint32
            | K::Fixed32
            | K::Uint64
            | K::Fixed64
            | K::Float
            | K::Double
            | K::Enum(_)
    )
}

pub(super) fn decode_packed_values(
    kind: &GrpcValueKind,
    mut bytes: bytes::Bytes,
) -> std::result::Result<Vec<wrkr_value::Value>, String> {
    use GrpcValueKind as K;

    let mut out: Vec<wrkr_value::Value> = Vec::new();

    match kind {
        K::Bool => {
            while bytes.has_remaining() {
                out.push(wrkr_value::Value::Bool(read_variant(&mut bytes)? != 0));
            }
        }
        K::Int32 | K::Int64 | K::Enum(_) => {
            while bytes.has_remaining() {
                out.push(wrkr_value::Value::I64(read_variant(&mut bytes)? as i64));
            }
        }
        K::Sint32 | K::Sint64 => {
            while bytes.has_remaining() {
                out.push(wrkr_value::Value::I64(decode_zigzag64(read_variant(
                    &mut bytes,
                )?)));
            }
        }
        K::Uint32 | K::Uint64 => {
            while bytes.has_remaining() {
                out.push(wrkr_value::Value::U64(read_variant(&mut bytes)?));
            }
        }

        K::Fixed32 => {
            while bytes.has_remaining() {
                if bytes.remaining() < 4 {
                    return Err("unexpected EOF reading packed fixed32".to_string());
                }
                out.push(wrkr_value::Value::U64(u64::from(bytes.get_u32_le())));
            }
        }
        K::Sfixed32 => {
            while bytes.has_remaining() {
                if bytes.remaining() < 4 {
                    return Err("unexpected EOF reading packed sfixed32".to_string());
                }
                out.push(wrkr_value::Value::I64(i64::from(bytes.get_i32_le())));
            }
        }
        K::Float => {
            while bytes.has_remaining() {
                if bytes.remaining() < 4 {
                    return Err("unexpected EOF reading packed float".to_string());
                }
                let v = bytes.get_u32_le();
                out.push(wrkr_value::Value::F64(f64::from(f32::from_bits(v))));
            }
        }

        K::Fixed64 => {
            while bytes.has_remaining() {
                if bytes.remaining() < 8 {
                    return Err("unexpected EOF reading packed fixed64".to_string());
                }
                out.push(wrkr_value::Value::U64(bytes.get_u64_le()));
            }
        }
        K::Sfixed64 => {
            while bytes.has_remaining() {
                if bytes.remaining() < 8 {
                    return Err("unexpected EOF reading packed sfixed64".to_string());
                }
                out.push(wrkr_value::Value::I64(bytes.get_i64_le()));
            }
        }
        K::Double => {
            while bytes.has_remaining() {
                if bytes.remaining() < 8 {
                    return Err("unexpected EOF reading packed double".to_string());
                }
                out.push(wrkr_value::Value::F64(f64::from_bits(bytes.get_u64_le())));
            }
        }

        K::String | K::Bytes | K::Message(_) => {
            return Err("packed encoding is not valid for this field type".to_string());
        }
    }

    Ok(out)
}
