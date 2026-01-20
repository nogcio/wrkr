use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq)]
pub enum SharedValue {
    Null,
    Bool(bool),
    I64(i64),
    F64(f64),
    String(Arc<str>),
    Array(Vec<SharedValue>),
    Object(HashMap<Arc<str>, SharedValue>),
}

impl SharedValue {
    #[must_use]
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Self::I64(v) => Some(*v),
            _ => None,
        }
    }
}
