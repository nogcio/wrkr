use parking_lot::RwLock;
use std::collections::HashMap;
use std::hash::Hash;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct KeyId(u32);

impl KeyId {
    pub const EMPTY: KeyId = KeyId(0);
}

impl From<u32> for KeyId {
    fn from(v: u32) -> Self {
        Self(v)
    }
}

impl From<KeyId> for u32 {
    fn from(v: KeyId) -> Self {
        v.0
    }
}

#[derive(Default, Debug)]
pub struct Interner {
    map: RwLock<HashMap<Arc<str>, u32>>,
    vec: RwLock<Vec<Arc<str>>>,
}

impl Interner {
    pub fn new() -> Self {
        Self {
            map: RwLock::new(HashMap::new()),
            vec: RwLock::new(Vec::new()),
        }
    }

    pub fn get_or_intern(&self, s: &str) -> KeyId {
        {
            let map = self.map.read();
            if let Some(&id) = map.get(s) {
                return KeyId(id);
            }
        }

        let mut map = self.map.write();
        let mut vec = self.vec.write();

        // Check again to avoid race
        if let Some(&id) = map.get(s) {
            return KeyId(id);
        }

        let id = vec.len() as u32;
        let s: Arc<str> = Arc::from(s);
        vec.push(s.clone());
        map.insert(s, id);

        KeyId(id)
    }

    pub fn resolve(&self, id: KeyId) -> Option<Arc<str>> {
        let vec = self.vec.read();
        vec.get(id.0 as usize).cloned()
    }
}
