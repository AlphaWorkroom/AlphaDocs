//! Snapshot-friendly storage. The serialized representation remains a plain
//! array; shared ownership never leaks into the document file format.
use serde::{Deserialize, Serialize};
use std::{
    ops::{Deref, DerefMut},
    sync::Arc,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SharedVec<T>(Arc<Vec<T>>);
impl<T: PartialEq> PartialEq for SharedVec<T> {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0) || self.0.as_slice() == other.0.as_slice()
    }
}
impl<T> Default for SharedVec<T> {
    fn default() -> Self {
        Self(Arc::new(vec![]))
    }
}
impl<T> From<Vec<T>> for SharedVec<T> {
    fn from(value: Vec<T>) -> Self {
        Self(Arc::new(value))
    }
}
impl<T> FromIterator<T> for SharedVec<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        iter.into_iter().collect::<Vec<_>>().into()
    }
}
impl<T> Deref for SharedVec<T> {
    type Target = Vec<T>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl<T: Clone> DerefMut for SharedVec<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        Arc::make_mut(&mut self.0)
    }
}
impl<T: Clone> IntoIterator for SharedVec<T> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<T>;
    fn into_iter(self) -> Self::IntoIter {
        Arc::try_unwrap(self.0)
            .unwrap_or_else(|shared| (*shared).clone())
            .into_iter()
    }
}
impl<'a, T> IntoIterator for &'a SharedVec<T> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
impl<'a, T: Clone> IntoIterator for &'a mut SharedVec<T> {
    type Item = &'a mut T;
    type IntoIter = std::slice::IterMut<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn snapshots_share_storage_until_mutated_and_serialize_as_arrays() {
        let original: SharedVec<u8> = vec![1, 2, 3].into();
        let mut edited = original.clone();
        assert!(Arc::ptr_eq(&original.0, &edited.0));
        edited.push(4);
        assert_eq!(original.as_slice(), [1, 2, 3]);
        assert_eq!(edited.as_slice(), [1, 2, 3, 4]);
        assert!(!Arc::ptr_eq(&original.0, &edited.0));
        assert_eq!(serde_json::to_string(&original).unwrap(), "[1,2,3]");
        assert_eq!(
            serde_json::from_str::<SharedVec<u8>>("[1,2,3]").unwrap(),
            original
        );
    }
}
