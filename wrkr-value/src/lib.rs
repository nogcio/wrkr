use std::collections::HashMap;
use std::sync::Arc;

use bytes::Bytes;

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
    Object(HashMap<Arc<str>, Value>),
    Map(HashMap<MapKey, Value>),
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
