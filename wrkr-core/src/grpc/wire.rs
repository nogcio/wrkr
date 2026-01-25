mod coerce;
mod decode;
mod encode;
mod map;
mod packed;
mod primitives;
mod scalar;

use crate::proto::GrpcMethod;

pub(crate) fn encode_value_for_method(
    method: &GrpcMethod,
    value: &wrkr_value::Value,
) -> std::result::Result<bytes::Bytes, String> {
    let mut out = bytes::BytesMut::new();
    encode::encode_message(method.input_fields(), value, &mut out)?;
    Ok(out.freeze())
}

pub(crate) fn decode_value_for_method(
    method: &GrpcMethod,
    bytes: bytes::Bytes,
) -> std::result::Result<wrkr_value::Value, String> {
    decode::decode_message_for_method(method, bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::{GrpcFieldShape, GrpcValueKind};

    use primitives::{decode_zigzag64, encode_zigzag64};

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
        assert!(
            decode::merge_decoded_field(&mut out, &name, &shape, wrkr_value::Value::Map(m1))
                .is_ok()
        );

        let mut m2: wrkr_value::MapMap = wrkr_value::MapMap::new();
        m2.insert(
            wrkr_value::MapKey::String(std::sync::Arc::<str>::from("FR")),
            wrkr_value::Value::I64(20),
        );
        assert!(
            decode::merge_decoded_field(&mut out, &name, &shape, wrkr_value::Value::Map(m2))
                .is_ok()
        );

        let Some(wrkr_value::Value::Map(got)) = out.get(&name) else {
            panic!("expected a map value");
        };
        assert_eq!(got.len(), 2);
    }
}
