use bytes::Buf as _;

use crate::proto::{GrpcFieldShape, GrpcMethod};

use super::map::decode_map_entry_into_object;
use super::packed::{decode_packed_values, kind_is_packable};
use super::primitives::{WireType, read_variant, skip_value};
use super::scalar::decode_scalar_value;

pub(super) fn decode_message_for_method(
    method: &GrpcMethod,
    bytes: bytes::Bytes,
) -> std::result::Result<wrkr_value::Value, String> {
    let fields = method.output_fields();
    let by_number = method.output_field_index_by_number();

    let mut src = bytes;
    let mut out: wrkr_value::ObjectMap = wrkr_value::ObjectMap::with_capacity(fields.len());

    while src.has_remaining() {
        let tag = read_variant(&mut src)?;
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

pub(super) fn decode_message_for_meta(
    meta: &crate::proto::GrpcMessageMeta,
    bytes: bytes::Bytes,
) -> std::result::Result<wrkr_value::Value, String> {
    let by_number = meta.fields_by_number();
    let mut src = bytes;
    let mut out: wrkr_value::ObjectMap = wrkr_value::ObjectMap::with_capacity(by_number.len());

    while src.has_remaining() {
        let tag = read_variant(&mut src)?;
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

pub(super) fn merge_decoded_field(
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
                None => match v {
                    wrkr_value::Value::Array(items) => {
                        out.insert(name.clone(), wrkr_value::Value::Array(items));
                    }
                    other => {
                        out.insert(name.clone(), wrkr_value::Value::Array(vec![other]));
                    }
                },
                Some(wrkr_value::Value::Array(existing)) => match v {
                    wrkr_value::Value::Array(mut items) => {
                        existing.append(&mut items);
                    }
                    other => {
                        existing.push(other);
                    }
                },
                Some(existing) => {
                    let prev = std::mem::replace(existing, wrkr_value::Value::Null);
                    match v {
                        wrkr_value::Value::Array(mut items) => {
                            let mut out_items = Vec::with_capacity(1 + items.len());
                            out_items.push(prev);
                            out_items.append(&mut items);
                            *existing = wrkr_value::Value::Array(out_items);
                        }
                        other => {
                            *existing = wrkr_value::Value::Array(vec![prev, other]);
                        }
                    }
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
            if wire_type == WireType::Len && kind_is_packable(kind) {
                let bytes = super::primitives::read_len_delimited(src)?;
                let items = decode_packed_values(kind, bytes)?;
                Ok(wrkr_value::Value::Array(items))
            } else {
                decode_scalar_value(kind, wire_type, src)
            }
        }
        GrpcFieldShape::Map {
            key_kind,
            value_kind,
        } => {
            if wire_type != WireType::Len {
                return Err("map field must be length-delimited".to_string());
            }
            let bytes = super::primitives::read_len_delimited(src)?;
            // Temporarily leverage map decoding but we just want one entry.
            // But we can call map::decode_map_entry directly which returns (k, v).
            // decode_map_entry is in map.rs
            let (k, v) = super::map::decode_map_entry(key_kind, value_kind, bytes)?;
            let mut map: wrkr_value::MapMap = wrkr_value::MapMap::with_capacity(1);
            map.insert(k, v);
            Ok(wrkr_value::Value::Map(map))
        }
    }
}

#[cfg(test)]
mod tests {
    use bytes::BufMut as _;

    use super::*;
    use crate::proto::{GrpcFieldShape, GrpcValueKind};

    use super::super::primitives::write_variant;

    #[test]
    fn decode_packed_varint_list_int32() {
        // Field payload contains packed varints: [1, 2, 150]
        let mut payload = bytes::BytesMut::new();
        write_variant(1, &mut payload);
        write_variant(2, &mut payload);
        write_variant(150, &mut payload);

        // Surround with len delimiter as it appears on the wire for packed fields.
        let mut src = bytes::BytesMut::new();
        write_variant(payload.len() as u64, &mut src);
        src.put_slice(&payload);

        let mut src = src.freeze();

        let shape = GrpcFieldShape::List {
            kind: GrpcValueKind::Int32,
        };

        let got = match decode_field_value(&shape, WireType::Len, &mut src) {
            Ok(v) => v,
            Err(e) => panic!("decode_field_value failed: {e}"),
        };
        let wrkr_value::Value::Array(items) = got else {
            panic!("expected array");
        };

        assert_eq!(items.len(), 3);
        assert_eq!(items[0], wrkr_value::Value::I64(1));
        assert_eq!(items[1], wrkr_value::Value::I64(2));
        assert_eq!(items[2], wrkr_value::Value::I64(150));
    }

    #[test]
    fn merge_decoded_field_extends_list_with_packed_array() {
        let mut out: wrkr_value::ObjectMap = wrkr_value::ObjectMap::new();
        let name = std::sync::Arc::<str>::from("nums");
        let shape = GrpcFieldShape::List {
            kind: GrpcValueKind::Int32,
        };

        if let Err(e) = merge_decoded_field(
            &mut out,
            &name,
            &shape,
            wrkr_value::Value::Array(vec![wrkr_value::Value::I64(1), wrkr_value::Value::I64(2)]),
        ) {
            panic!("merge_decoded_field failed: {e}");
        }

        if let Err(e) = merge_decoded_field(&mut out, &name, &shape, wrkr_value::Value::I64(3)) {
            panic!("merge_decoded_field failed: {e}");
        }

        let Some(wrkr_value::Value::Array(items)) = out.get(&name) else {
            panic!("expected array");
        };
        assert_eq!(items.len(), 3);
        assert_eq!(items[0], wrkr_value::Value::I64(1));
        assert_eq!(items[1], wrkr_value::Value::I64(2));
        assert_eq!(items[2], wrkr_value::Value::I64(3));
    }
}
