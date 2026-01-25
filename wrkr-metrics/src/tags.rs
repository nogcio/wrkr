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
}
