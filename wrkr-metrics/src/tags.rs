use crate::key::KeyId;
use smallvec::SmallVec;

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct TagSet {
    // SmallVec to avoid allocation for small tag sets (usually < 4)
    pub(crate) tags: SmallVec<[(KeyId, KeyId); 4]>,
}

impl TagSet {
    pub fn from_sorted_iter(iter: impl IntoIterator<Item = (KeyId, KeyId)>) -> Self {
        Self {
            tags: iter.into_iter().collect(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.tags.is_empty()
    }

    pub fn len(&self) -> usize {
        self.tags.len()
    }

    pub fn iter(&self) -> impl Iterator<Item = (KeyId, KeyId)> + '_ {
        self.tags.iter().copied()
    }

    pub fn contains(&self, key: KeyId, value: KeyId) -> bool {
        self.tags
            .binary_search_by(|(k, v)| (*k, *v).cmp(&(key, value)))
            .is_ok()
    }

    pub fn get(&self, key: KeyId) -> Option<KeyId> {
        let slice: &[(KeyId, KeyId)] = &self.tags;
        let idx = slice.partition_point(|(k, _)| *k < key);
        slice.get(idx).and_then(|(k, v)| (*k == key).then_some(*v))
    }

    pub fn project(&self, keys: &[KeyId]) -> TagSet {
        if keys.is_empty() {
            return TagSet::default();
        }

        let mut out = SmallVec::<[(KeyId, KeyId); 4]>::new();
        for key in keys {
            if let Some(value) = self.get(*key) {
                out.push((*key, value));
            }
        }

        TagSet { tags: out }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tagset_contains_and_get() {
        let a = KeyId::from(1);
        let b = KeyId::from(2);
        let c = KeyId::from(3);

        let set = TagSet::from_sorted_iter([(a, b), (c, a)]);
        assert!(set.contains(a, b));
        assert!(!set.contains(a, a));
        assert_eq!(set.get(a), Some(b));
        assert_eq!(set.get(c), Some(a));
        assert_eq!(set.get(b), None);
    }
}
