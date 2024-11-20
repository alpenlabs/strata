use std::cmp::Ordering;

use borsh::{BorshDeserialize, BorshSerialize};

/// A vector wrapper that ensures the elements are sorted
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct SortedVec<T> {
    inner: Vec<T>,
}

impl<T: Ord + Clone> SortedVec<T> {
    /// Creates a new, empty `SortedVec`.
    pub fn new() -> Self {
        Self { inner: Vec::new() }
    }

    /// Inserts an element into the `SortedVec`, maintaining sorted order. This runs in O(n) because
    /// of shifting of elements.
    pub fn insert(&mut self, value: T) {
        let pos = self.inner.binary_search(&value).unwrap_or_else(|e| e);
        self.inner.insert(pos, value);
    }

    /// Removes an element from the `SortedVec`. Returns `true` if the element was found and
    /// removed. This runs in O(n) due to shifting of elements.
    pub fn remove(&mut self, value: &T) -> bool {
        if let Ok(pos) = self.inner.binary_search(value) {
            self.inner.remove(pos);
            true
        } else {
            false
        }
    }

    /// Checks if the `SortedVec` contains the given value.
    pub fn contains(&self, value: &T) -> bool {
        self.binary_search(value).is_ok()
    }

    /// Perform binary search on the vector
    pub fn binary_search(&self, value: &T) -> Result<usize, usize> {
        self.inner.binary_search(value)
    }

    /// Returns the number of elements in the `SortedVec`.
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Returns `true` if the `SortedVec` is empty.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Returns a reference to the element at the given index.
    pub fn get(&self, index: usize) -> Option<&T> {
        self.inner.get(index)
    }

    /// Exposes `iter` method of the inner vector
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.inner.iter()
    }

    /// Consumes the `SortedVec` and returns the inner `Vec`.
    pub fn into_inner(self) -> Vec<T> {
        self.inner
    }

    /// Slice
    pub fn as_slice(&self) -> &[T] {
        &self.inner
    }

    /// Merge another `SortedVec`
    pub fn merge(&mut self, other: &Self) {
        let mut merged: Vec<T> = Vec::with_capacity(self.len() + other.len());
        let mut i = 0;
        let mut j = 0;
        while i < self.len() && j < other.len() {
            match self.inner[i].cmp(&other.inner[j]) {
                Ordering::Greater => {
                    merged.push(other.inner[j].clone());
                    j += 1;
                }
                _ => {
                    merged.push(self.inner[i].clone());
                    i += 1;
                }
            }
        }

        // Append remaining elements
        merged.extend_from_slice(&self.inner[i..]);
        merged.extend_from_slice(&other.inner[j..]);

        self.inner = merged;
    }
}

impl<T: Clone> SortedVec<T> {
    /// Converts to vec
    pub fn to_vec(&self) -> Vec<T> {
        self.inner.clone()
    }
}

impl<T: Ord + Clone> Default for SortedVec<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Ord + Clone> From<Vec<T>> for SortedVec<T> {
    /// Creates a `SortedVec` from a `Vec`, sorting the elements.
    fn from(mut vec: Vec<T>) -> Self {
        vec.sort();
        Self { inner: vec }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_and_sorted_order() {
        let mut sorted_vec = SortedVec::new();
        sorted_vec.insert(5);
        sorted_vec.insert(3);
        sorted_vec.insert(8);

        assert_eq!(&sorted_vec.to_vec(), &[3, 5, 8]); // Check the sorted order
    }

    #[test]
    fn test_remove_existing() {
        let mut sorted_vec = SortedVec::from(vec![3, 5, 8]);
        let removed = sorted_vec.remove(&5);

        assert!(removed);
        assert_eq!(&sorted_vec.to_vec(), &[3, 8]); // Check the sorted order after removal
    }

    #[test]
    fn test_remove_non_existing() {
        let mut sorted_vec = SortedVec::from(vec![3, 5, 8]);
        let removed = sorted_vec.remove(&10);

        assert!(!removed); // Removing a non-existing element
        assert_eq!(sorted_vec.to_vec(), &[3, 5, 8]);
    }

    #[test]
    fn test_contains() {
        let sorted_vec = SortedVec::from(vec![3, 5, 8]);

        assert!(sorted_vec.contains(&5));
        assert!(!sorted_vec.contains(&10));
    }

    #[test]
    fn test_len_and_empty() {
        let mut sorted_vec = SortedVec::new();

        assert!(sorted_vec.is_empty());
        assert_eq!(sorted_vec.len(), 0);

        sorted_vec.insert(5);
        assert!(!sorted_vec.is_empty());
        assert_eq!(sorted_vec.len(), 1);
    }

    #[test]
    fn test_merge() {
        // Create vectors with duplicate elements
        let mut sv1 = SortedVec::new();
        sv1.insert(1);
        sv1.insert(3);
        sv1.insert(4);
        sv1.insert(4);
        sv1.insert(5);

        let mut sv2 = SortedVec::new();
        sv2.insert(3);
        sv2.insert(5);
        sv2.insert(7);
        sv2.insert(8);

        sv1.merge(&sv2);

        assert_eq!(sv1.as_slice(), &[1, 3, 3, 4, 4, 5, 5, 7, 8]);
    }
}
