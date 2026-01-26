use crate::proto::{GrpcFieldShape, GrpcInputFieldMeta};

use super::map::encode_map_entry;
use super::scalar::encode_scalar_field;

pub(super) fn encode_message(
    fields_by_name: &std::collections::HashMap<std::sync::Arc<str>, GrpcInputFieldMeta>,
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

#[cfg(test)]
mod tests {
    use super::*;

    use crate::proto::GrpcValueKind;

    use bytes::Buf as _;
    use std::sync::Arc;

    fn test_descriptor_pool() -> prost_reflect::DescriptorPool {
        use prost_reflect::DescriptorPool;
        use prost_types::{
            DescriptorProto, FieldDescriptorProto, FileDescriptorProto, FileDescriptorSet,
            field_descriptor_proto::{Label, Type},
        };

        let msg = DescriptorProto {
            name: Some("Msg".to_string()),
            field: vec![
                FieldDescriptorProto {
                    name: Some("a".to_string()),
                    number: Some(1),
                    label: Some(Label::Optional as i32),
                    r#type: Some(Type::Int64 as i32),
                    ..Default::default()
                },
                FieldDescriptorProto {
                    name: Some("list".to_string()),
                    number: Some(2),
                    label: Some(Label::Repeated as i32),
                    r#type: Some(Type::Int64 as i32),
                    ..Default::default()
                },
                FieldDescriptorProto {
                    name: Some("m".to_string()),
                    number: Some(3),
                    label: Some(Label::Optional as i32),
                    r#type: Some(Type::Bytes as i32),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };

        let file = FileDescriptorProto {
            name: Some("test.proto".to_string()),
            package: Some("test".to_string()),
            message_type: vec![msg],
            syntax: Some("proto3".to_string()),
            ..Default::default()
        };

        let fds = FileDescriptorSet { file: vec![file] };
        let Ok(pool) = DescriptorPool::from_file_descriptor_set(fds) else {
            panic!("failed to build descriptor pool");
        };
        pool
    }

    fn msg_field(
        pool: &prost_reflect::DescriptorPool,
        name: &str,
    ) -> prost_reflect::FieldDescriptor {
        let Some(msg) = pool.get_message_by_name("test.Msg") else {
            panic!("message not found");
        };
        let Some(field) = msg.get_field_by_name(name) else {
            panic!("field not found: {name}");
        };
        field
    }

    #[test]
    fn encode_message_errors_on_unknown_field_in_object() {
        let fields_by_name: std::collections::HashMap<Arc<str>, GrpcInputFieldMeta> =
            std::collections::HashMap::new();

        let mut obj = wrkr_value::ObjectMap::new();
        obj.insert(Arc::<str>::from("nope"), wrkr_value::Value::Null);

        let mut out = bytes::BytesMut::new();
        let got = encode_message(&fields_by_name, &wrkr_value::Value::Object(obj), &mut out);
        assert!(got.is_err());
    }

    #[test]
    fn encode_message_map_requires_string_field_names() {
        let fields_by_name: std::collections::HashMap<Arc<str>, GrpcInputFieldMeta> =
            std::collections::HashMap::new();

        let mut m = wrkr_value::MapMap::new();
        m.insert(wrkr_value::MapKey::I64(1), wrkr_value::Value::Null);

        let mut out = bytes::BytesMut::new();
        let got = encode_message(&fields_by_name, &wrkr_value::Value::Map(m), &mut out);
        assert!(got.is_err());
    }

    #[test]
    fn encode_field_scalar_and_list_and_map_branches() {
        let pool = test_descriptor_pool();

        let field_a = msg_field(&pool, "a");
        let field_list = msg_field(&pool, "list");
        let field_m = msg_field(&pool, "m");

        let mut fields_by_name: std::collections::HashMap<Arc<str>, GrpcInputFieldMeta> =
            std::collections::HashMap::new();

        fields_by_name.insert(
            Arc::<str>::from("a"),
            GrpcInputFieldMeta {
                field: field_a,
                shape: GrpcFieldShape::Scalar {
                    kind: GrpcValueKind::Int64,
                },
            },
        );
        fields_by_name.insert(
            Arc::<str>::from("list"),
            GrpcInputFieldMeta {
                field: field_list,
                shape: GrpcFieldShape::List {
                    kind: GrpcValueKind::Int64,
                },
            },
        );
        fields_by_name.insert(
            Arc::<str>::from("m"),
            GrpcInputFieldMeta {
                field: field_m,
                shape: GrpcFieldShape::Map {
                    key_kind: prost_reflect::Kind::String,
                    value_kind: GrpcValueKind::Int64,
                },
            },
        );

        // Scalar encodes: a=5 => tag(1,varint)=8, value=5
        let mut obj = wrkr_value::ObjectMap::new();
        obj.insert(Arc::<str>::from("a"), wrkr_value::Value::I64(5));

        let mut out = bytes::BytesMut::new();
        assert!(encode_message(&fields_by_name, &wrkr_value::Value::Object(obj), &mut out).is_ok());
        let mut bytes = out.freeze();
        assert_eq!(bytes.get_u8(), 0x08);
        assert_eq!(bytes.get_u8(), 0x05);

        // List requires array.
        let mut obj = wrkr_value::ObjectMap::new();
        obj.insert(Arc::<str>::from("list"), wrkr_value::Value::I64(1));
        let mut out = bytes::BytesMut::new();
        assert!(
            encode_message(&fields_by_name, &wrkr_value::Value::Object(obj), &mut out).is_err()
        );

        // List encodes multiple values: list=[1,2]
        let mut obj = wrkr_value::ObjectMap::new();
        obj.insert(
            Arc::<str>::from("list"),
            wrkr_value::Value::Array(vec![wrkr_value::Value::I64(1), wrkr_value::Value::I64(2)]),
        );
        let mut out = bytes::BytesMut::new();
        assert!(encode_message(&fields_by_name, &wrkr_value::Value::Object(obj), &mut out).is_ok());
        assert_eq!(out.as_ref(), b"\x10\x01\x10\x02");

        // Map requires map/object.
        let mut obj = wrkr_value::ObjectMap::new();
        obj.insert(Arc::<str>::from("m"), wrkr_value::Value::I64(1));
        let mut out = bytes::BytesMut::new();
        assert!(
            encode_message(&fields_by_name, &wrkr_value::Value::Object(obj), &mut out).is_err()
        );

        // Map from object encodes at least one entry tag (field 3, len-delimited => 0x1a).
        let mut inner = wrkr_value::ObjectMap::new();
        inner.insert(Arc::<str>::from("k"), wrkr_value::Value::I64(7));

        let mut obj = wrkr_value::ObjectMap::new();
        obj.insert(Arc::<str>::from("m"), wrkr_value::Value::Object(inner));

        let mut out = bytes::BytesMut::new();
        assert!(encode_message(&fields_by_name, &wrkr_value::Value::Object(obj), &mut out).is_ok());
        assert!(out.as_ref().contains(&0x1a));
    }
}
