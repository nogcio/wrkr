use crate::proto::GrpcFieldShape;

use super::map::encode_map_entry;
use super::scalar::encode_scalar_field;

pub(super) fn encode_message(
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
