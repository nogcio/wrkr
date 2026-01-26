mod coerce;
mod decode;
mod encode;
mod map;
mod packed;
mod primitives;
mod scalar;

use crate::GrpcMethod;

fn wire_type_for_kind(kind: &crate::proto::GrpcValueKind) -> primitives::WireType {
    use crate::proto::GrpcValueKind as K;
    use primitives::WireType;

    match kind {
        K::Bool
        | K::Int32
        | K::Sint32
        | K::Int64
        | K::Sint64
        | K::Uint32
        | K::Uint64
        | K::Enum(_) => WireType::Varint,

        K::Fixed32 | K::Sfixed32 | K::Float => WireType::ThirtyTwoBit,

        K::Fixed64 | K::Sfixed64 | K::Double => WireType::SixtyFourBit,

        K::String | K::Bytes | K::Message(_) => WireType::Len,
    }
}

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

    use bytes::{Buf as _, BufMut as _, BytesMut};
    use std::sync::Arc;

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

    #[test]
    fn coerce_to_bool_covers_common_inputs() {
        assert!(coerce::to_bool(&wrkr_value::Value::Bool(true)));
        assert!(!coerce::to_bool(&wrkr_value::Value::Bool(false)));

        assert!(coerce::to_bool(&wrkr_value::Value::String(Arc::from(
            "true"
        ))));
        assert!(coerce::to_bool(&wrkr_value::Value::String(Arc::from("1"))));
        assert!(!coerce::to_bool(&wrkr_value::Value::String(Arc::from(
            "false"
        ))));

        assert!(coerce::to_bool(&wrkr_value::Value::I64(123)));
        assert!(!coerce::to_bool(&wrkr_value::Value::I64(0)));
        assert!(coerce::to_bool(&wrkr_value::Value::U64(1)));
        assert!(!coerce::to_bool(&wrkr_value::Value::U64(0)));
        assert!(!coerce::to_bool(&wrkr_value::Value::Null));
    }

    #[test]
    fn coerce_map_keys_to_ints() {
        let Ok(got) = coerce::map_key_to_i64(wrkr_value::MapKey::I64(-7)) else {
            panic!("expected i64 map key");
        };
        assert_eq!(got, -7);

        let Ok(got) = coerce::map_key_to_i64(wrkr_value::MapKey::U64(7)) else {
            panic!("expected i64 map key");
        };
        assert_eq!(got, 7);

        let Ok(got) = coerce::map_key_to_i64(wrkr_value::MapKey::String(Arc::from("42"))) else {
            panic!("expected i64 map key");
        };
        assert_eq!(got, 42);
        assert!(coerce::map_key_to_i64(wrkr_value::MapKey::String(Arc::from("nope"))).is_err());

        let Ok(got) = coerce::map_key_to_u64(wrkr_value::MapKey::U64(7)) else {
            panic!("expected u64 map key");
        };
        assert_eq!(got, 7);

        let Ok(got) = coerce::map_key_to_u64(wrkr_value::MapKey::I64(7)) else {
            panic!("expected u64 map key");
        };
        assert_eq!(got, 7);

        assert!(coerce::map_key_to_u64(wrkr_value::MapKey::I64(-1)).is_err());

        let Ok(got) = coerce::map_key_to_u64(wrkr_value::MapKey::String(Arc::from("42"))) else {
            panic!("expected u64 map key");
        };
        assert_eq!(got, 42);
        assert!(coerce::map_key_to_u64(wrkr_value::MapKey::String(Arc::from("nope"))).is_err());
    }

    #[test]
    fn coerce_lossy_conversions() {
        assert_eq!(
            coerce::to_string_lossy(&wrkr_value::Value::String(Arc::from("hi"))),
            "hi"
        );
        assert_eq!(coerce::to_string_lossy(&wrkr_value::Value::I64(-1)), "-1");
        assert_eq!(coerce::to_string_lossy(&wrkr_value::Value::U64(1)), "1");
        assert_eq!(
            coerce::to_string_lossy(&wrkr_value::Value::Bool(true)),
            "true"
        );
        assert_eq!(coerce::to_string_lossy(&wrkr_value::Value::Null), "");

        let b = bytes::Bytes::from_static(b"abc");
        assert_eq!(
            coerce::to_bytes_lossy(&wrkr_value::Value::Bytes(b.clone())),
            b
        );
        assert_eq!(
            coerce::to_bytes_lossy(&wrkr_value::Value::String(Arc::from("abc"))),
            bytes::Bytes::from_static(b"abc")
        );
        assert_eq!(
            coerce::to_bytes_lossy(&wrkr_value::Value::Null),
            bytes::Bytes::new()
        );
    }

    #[test]
    fn packed_kind_is_packable_matches_protobuf_rules() {
        assert!(packed::kind_is_packable(&GrpcValueKind::Bool));
        assert!(packed::kind_is_packable(&GrpcValueKind::Int64));
        assert!(packed::kind_is_packable(&GrpcValueKind::Float));
        assert!(!packed::kind_is_packable(&GrpcValueKind::String));
        assert!(!packed::kind_is_packable(&GrpcValueKind::Bytes));
    }

    #[test]
    fn decode_packed_values_varint_kinds() {
        let mut buf = BytesMut::new();
        primitives::write_variant(0, &mut buf);
        primitives::write_variant(1, &mut buf);
        let Ok(got) = packed::decode_packed_values(&GrpcValueKind::Bool, buf.freeze()) else {
            panic!("expected packed bool decode");
        };
        assert_eq!(
            got,
            vec![
                wrkr_value::Value::Bool(false),
                wrkr_value::Value::Bool(true)
            ]
        );

        let mut buf = BytesMut::new();
        primitives::write_variant(123, &mut buf);
        primitives::write_variant(0, &mut buf);
        let Ok(got) = packed::decode_packed_values(&GrpcValueKind::Int64, buf.freeze()) else {
            panic!("expected packed int64 decode");
        };
        assert_eq!(
            got,
            vec![wrkr_value::Value::I64(123), wrkr_value::Value::I64(0)]
        );

        let mut buf = BytesMut::new();
        primitives::write_variant(primitives::encode_zigzag64(-1), &mut buf);
        primitives::write_variant(primitives::encode_zigzag64(1), &mut buf);
        let Ok(got) = packed::decode_packed_values(&GrpcValueKind::Sint64, buf.freeze()) else {
            panic!("expected packed sint64 decode");
        };
        assert_eq!(
            got,
            vec![wrkr_value::Value::I64(-1), wrkr_value::Value::I64(1)]
        );

        let mut buf = BytesMut::new();
        primitives::write_variant(7, &mut buf);
        primitives::write_variant(9, &mut buf);
        let Ok(got) = packed::decode_packed_values(&GrpcValueKind::Uint64, buf.freeze()) else {
            panic!("expected packed uint64 decode");
        };
        assert_eq!(
            got,
            vec![wrkr_value::Value::U64(7), wrkr_value::Value::U64(9)]
        );
    }

    #[test]
    fn decode_packed_values_fixed_width_kinds() {
        let mut buf = BytesMut::new();
        buf.put_u32_le(123);
        buf.put_u32_le(0);
        let Ok(got) = packed::decode_packed_values(&GrpcValueKind::Fixed32, buf.freeze()) else {
            panic!("expected packed fixed32 decode");
        };
        assert_eq!(
            got,
            vec![wrkr_value::Value::U64(123), wrkr_value::Value::U64(0)]
        );

        let mut buf = BytesMut::new();
        buf.put_i32_le(-7);
        buf.put_i32_le(7);
        let Ok(got) = packed::decode_packed_values(&GrpcValueKind::Sfixed32, buf.freeze()) else {
            panic!("expected packed sfixed32 decode");
        };
        assert_eq!(
            got,
            vec![wrkr_value::Value::I64(-7), wrkr_value::Value::I64(7)]
        );

        let mut buf = BytesMut::new();
        buf.put_u32_le((1.5_f32).to_bits());
        let Ok(got) = packed::decode_packed_values(&GrpcValueKind::Float, buf.freeze()) else {
            panic!("expected packed float decode");
        };
        assert_eq!(got.len(), 1);
        let wrkr_value::Value::F64(v) = got[0] else {
            panic!("expected f64");
        };
        assert!((v - 1.5).abs() < 1e-9);

        let mut buf = BytesMut::new();
        buf.put_u64_le(123);
        buf.put_u64_le(0);
        let Ok(got) = packed::decode_packed_values(&GrpcValueKind::Fixed64, buf.freeze()) else {
            panic!("expected packed fixed64 decode");
        };
        assert_eq!(
            got,
            vec![wrkr_value::Value::U64(123), wrkr_value::Value::U64(0)]
        );

        let mut buf = BytesMut::new();
        buf.put_i64_le(-7);
        buf.put_i64_le(7);
        let Ok(got) = packed::decode_packed_values(&GrpcValueKind::Sfixed64, buf.freeze()) else {
            panic!("expected packed sfixed64 decode");
        };
        assert_eq!(
            got,
            vec![wrkr_value::Value::I64(-7), wrkr_value::Value::I64(7)]
        );

        let mut buf = BytesMut::new();
        buf.put_u64_le((1.5_f64).to_bits());
        let Ok(got) = packed::decode_packed_values(&GrpcValueKind::Double, buf.freeze()) else {
            panic!("expected packed double decode");
        };
        assert_eq!(got, vec![wrkr_value::Value::F64(1.5)]);
    }

    #[test]
    fn decode_packed_values_rejects_non_packable_kinds() {
        let got = packed::decode_packed_values(
            &GrpcValueKind::String,
            bytes::Bytes::from_static(b"ignored"),
        );
        assert!(got.is_err());
    }

    #[test]
    fn decode_packed_values_errors_on_truncated_fixed_width() {
        let got = packed::decode_packed_values(
            &GrpcValueKind::Fixed32,
            bytes::Bytes::from_static(b"\x01\x02\x03"),
        );
        assert!(got.is_err());

        let got = packed::decode_packed_values(
            &GrpcValueKind::Fixed64,
            bytes::Bytes::from_static(b"\x01\x02\x03\x04\x05\x06\x07"),
        );
        assert!(got.is_err());
    }

    #[test]
    fn decode_packed_values_errors_on_invalid_varint() {
        // Continuation bit set but no terminating byte.
        let got =
            packed::decode_packed_values(&GrpcValueKind::Bool, bytes::Bytes::from_static(b"\x80"));
        assert!(got.is_err());
    }

    #[test]
    fn decode_scalar_value_string_bytes_and_errors() {
        // String, valid UTF-8.
        let mut buf = BytesMut::new();
        primitives::write_len_delimited(bytes::Bytes::from_static(b"hi"), &mut buf);
        let mut src = buf.freeze();
        let Ok(got) = scalar::decode_scalar_value(
            &GrpcValueKind::String,
            primitives::WireType::Len,
            &mut src,
        ) else {
            panic!("expected string decode");
        };
        assert_eq!(got, wrkr_value::Value::String(Arc::from("hi")));

        // String, invalid UTF-8 => empty string.
        let mut buf = BytesMut::new();
        primitives::write_len_delimited(bytes::Bytes::from_static(b"\xff"), &mut buf);
        let mut src = buf.freeze();
        let Ok(got) = scalar::decode_scalar_value(
            &GrpcValueKind::String,
            primitives::WireType::Len,
            &mut src,
        ) else {
            panic!("expected string decode");
        };
        assert_eq!(got, wrkr_value::Value::String(Arc::from("")));

        // Bytes.
        let mut buf = BytesMut::new();
        primitives::write_len_delimited(bytes::Bytes::from_static(b"abc"), &mut buf);
        let mut src = buf.freeze();
        let Ok(got) =
            scalar::decode_scalar_value(&GrpcValueKind::Bytes, primitives::WireType::Len, &mut src)
        else {
            panic!("expected bytes decode");
        };
        assert_eq!(
            got,
            wrkr_value::Value::Bytes(bytes::Bytes::from_static(b"abc"))
        );

        // Wrong wire type errors.
        let mut src = bytes::Bytes::from_static(b"");
        assert!(
            scalar::decode_scalar_value(&GrpcValueKind::Bool, primitives::WireType::Len, &mut src,)
                .is_err()
        );
    }

    #[test]
    fn decode_scalar_value_fixed_width_eof_errors() {
        let mut src = bytes::Bytes::from_static(b"\x01\x02\x03");
        assert!(
            scalar::decode_scalar_value(
                &GrpcValueKind::Fixed32,
                primitives::WireType::ThirtyTwoBit,
                &mut src,
            )
            .is_err()
        );

        let mut src = bytes::Bytes::from_static(b"\x01\x02\x03\x04\x05\x06\x07");
        assert!(
            scalar::decode_scalar_value(
                &GrpcValueKind::Fixed64,
                primitives::WireType::SixtyFourBit,
                &mut src,
            )
            .is_err()
        );
    }

    #[test]
    fn encode_scalar_field_writes_expected_wire_types_and_values() {
        fn read_tag(bytes: &mut bytes::Bytes) -> u64 {
            let Ok(tag) = primitives::read_variant(bytes) else {
                panic!("expected tag varint");
            };
            tag
        }

        // Bool.
        let mut out = BytesMut::new();
        assert!(
            scalar::encode_scalar_field(
                1,
                &GrpcValueKind::Bool,
                &wrkr_value::Value::Bool(true),
                &mut out
            )
            .is_ok()
        );
        let mut bytes = out.freeze();
        assert_eq!(read_tag(&mut bytes), 1_u64 << 3);
        let Ok(v) = primitives::read_variant(&mut bytes) else {
            panic!("expected bool value");
        };
        assert_eq!(v, 1);

        // String.
        let mut out = BytesMut::new();
        assert!(
            scalar::encode_scalar_field(
                2,
                &GrpcValueKind::String,
                &wrkr_value::Value::String(Arc::from("hi")),
                &mut out
            )
            .is_ok()
        );
        let mut bytes = out.freeze();
        assert_eq!(read_tag(&mut bytes), (2_u64 << 3) | 2);
        let Ok(b) = primitives::read_len_delimited(&mut bytes) else {
            panic!("expected len-delimited string");
        };
        assert_eq!(b, bytes::Bytes::from_static(b"hi"));

        // Bytes.
        let mut out = BytesMut::new();
        assert!(
            scalar::encode_scalar_field(
                3,
                &GrpcValueKind::Bytes,
                &wrkr_value::Value::Bytes(bytes::Bytes::from_static(b"abc")),
                &mut out
            )
            .is_ok()
        );
        let mut bytes = out.freeze();
        assert_eq!(read_tag(&mut bytes), (3_u64 << 3) | 2);
        let Ok(b) = primitives::read_len_delimited(&mut bytes) else {
            panic!("expected len-delimited bytes");
        };
        assert_eq!(b, bytes::Bytes::from_static(b"abc"));

        // Fixed32.
        let mut out = BytesMut::new();
        assert!(
            scalar::encode_scalar_field(
                4,
                &GrpcValueKind::Fixed32,
                &wrkr_value::Value::U64(10),
                &mut out
            )
            .is_ok()
        );
        let mut bytes = out.freeze();
        assert_eq!(read_tag(&mut bytes), (4_u64 << 3) | 5);
        assert_eq!(bytes.get_u32_le(), 10);

        // Error path: int conversion from null.
        let mut out = BytesMut::new();
        assert!(
            scalar::encode_scalar_field(
                5,
                &GrpcValueKind::Int64,
                &wrkr_value::Value::Null,
                &mut out
            )
            .is_err()
        );
    }
}
