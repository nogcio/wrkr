use std::sync::Arc;

use bytes::Bytes;

pub type ObjectMap = ahash::AHashMap<Arc<str>, Value>;
pub type MapMap = ahash::AHashMap<MapKey, Value>;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MapKey {
    Bool(bool),
    I64(i64),
    U64(u64),
    String(Arc<str>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    I64(i64),
    U64(u64),
    F64(f64),
    String(Arc<str>),
    Bytes(Bytes),
    Array(Vec<Value>),
    Object(ObjectMap),
    Map(MapMap),
}

impl Value {
    #[must_use]
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Self::I64(v) => Some(*v),
            _ => None,
        }
    }
}
