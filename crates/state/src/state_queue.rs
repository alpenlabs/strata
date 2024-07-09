//! Generic queue type used in CL state, serializable with Borsh.
//!
//! There's a few different types of entries in CL state that represent "queues"
//! of data that we push things into, have monotonic indexes, and pull out.
//! This module is meant to implement this bookkeeping generically.  This does
//! *not* use `VecDeque` internally, as it's designed around being easily
//! serializable.

use borsh::{BorshDeserialize, BorshSerialize};

#[derive(Clone, Debug, BorshDeserialize, BorshSerialize)]
pub struct StateQueue<T> {
    /// The front of the queue that we take entries out of.
    base_idx: u64,

    /// The entries in the queue.
    entries: Vec<T>,
}

impl<T> StateQueue<T> {
    /// Creates a new empty fresh queue.
    pub fn new_empty() -> Self {
        Self {
            base_idx: 0,
            entries: Vec::new(),
        }
    }

    /// Creates a new empty queue with a particular base offset.
    pub fn new_at_index(base_idx: u64) -> Self {
        Self {
            base_idx,
            entries: Vec::new(),
        }
    }

    /// Returns the "base index", which is the absolute position of the front of
    /// the queue, even if there is no element at the front of the queue.
    pub fn base_idx(&self) -> u64 {
        self.base_idx
    }

    /// Returns a slice over the entries in the queue, without their positioning
    /// information.  Consider if `.iter_entries` is more well-suited.
    pub fn entries(&self) -> &[T] {
        &self.entries
    }

    /// Returns the number of items in the queue.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns if the queue is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns the index of the element at the front of the queue, if there is
    /// one.
    pub fn front_idx(&self) -> Option<u64> {
        if !self.entries.is_empty() {
            Some(self.base_idx)
        } else {
            None
        }
    }

    /// Returns the index of the element at the back of the queue, if there is
    /// one.
    pub fn back_idx(&self) -> Option<u64> {
        self.front_idx().map(|i| i + self.entries.len() as u64 - 1)
    }

    /// Returns the absolute index of the next element to be written to the queue.
    pub fn next_idx(&self) -> u64 {
        self.base_idx + self.entries.len() as u64
    }

    /// Gets an entry by absolute position.
    pub fn get_absolute(&self, idx: u64) -> Option<&T> {
        if idx < self.base_idx || idx >= self.next_idx() {
            return None;
        }

        let off = idx - self.base_idx;
        Some(self.entries.get(off as usize).unwrap())
    }

    /// Pushes an entry to the back of the queue, returning the absolute
    /// position of the entry.
    pub fn push_back(&mut self, ent: T) -> u64 {
        let idx = self.next_idx();
        self.entries.push(ent);
        idx
    }

    /// Pops an entry from the back of the queue.
    pub fn pop_back(&mut self) -> Option<T> {
        self.entries.pop()
    }

    /// Pops the front of the queue.
    pub fn pop_front(&mut self) -> Option<T> {
        if !self.entries.is_empty() {
            self.base_idx += 1;
            Some(self.entries.remove(0))
        } else {
            None
        }
    }

    /// Pops the front N element of the queue, only if they can all be popped.
    ///
    /// This is a batch operation so that we don't have to repeatedly take
    /// single elements out of the vec with `.pop_front()` and move the later
    /// ones over if we want to take out multiple elements, and it provides a
    /// check to ensure that it can really be done atomically.
    pub fn pop_front_n<const N: usize>(&mut self) -> Option<[T; N]> {
        if self.entries.len() < N {
            return None;
        }

        // Split the queue entries we want to keep into its own vec.
        let out = {
            let mut new_entries = self.entries.split_off(N);
            std::mem::swap(&mut self.entries, &mut new_entries);
            new_entries
        };

        // TODO verify that this is safe
        let slice_box = out.into_boxed_slice();
        let slice_ptr = slice_box.as_ptr();
        assert_eq!(slice_box.len(), N);
        let arr_box = unsafe {
            std::mem::forget(slice_box);
            Box::<[T; N]>::from_raw(slice_ptr as *mut [T; N])
        };

        // Now just copy it to the stack.
        self.base_idx += N as u64;
        Some(*arr_box)
    }

    /// Iterates over the entries in the queue, from front to back.
    pub fn iter_entries<'q>(&'q self) -> impl Iterator<Item = (u64, &'q T)> {
        self.entries
            .iter()
            .enumerate()
            .map(|(i, e)| (i as u64 + self.base_idx, e))
    }
}

#[cfg(test)]
mod tests {
    use super::StateQueue;

    // TODO maybe add a queue that goes back and forth several times
    #[test]
    fn test_push_pop_arr() {
        let mut q = StateQueue::<u64>::new_empty();

        let i0 = q.push_back(5);
        assert_eq!(i0, 0);
        let i1 = q.push_back(6);
        assert_eq!(i1, 1);
        q.push_back(7);
        q.push_back(8);
        q.push_back(9);
        let i2a = q.push_back(10);
        assert_eq!(q.pop_back(), Some(10));
        let i2b = q.push_back(15);
        assert_eq!(i2a, i2b);
        q.push_back(20);
        q.push_back(25);

        eprintln!("queue: {q:?}");

        let n0 = q.next_idx();
        assert_eq!(n0, 8);

        let arr = q.pop_front_n::<3>();
        assert_eq!(arr, Some([5, 6, 7]));
        let b0 = q.base_idx();
        assert_eq!(b0, 3);

        let n1 = q.next_idx();
        assert_eq!(n1, 8);

        let a0 = q.pop_front();
        assert_eq!(a0, Some(8));
        let b1 = q.base_idx();
        assert_eq!(b1, 4);

        let n2 = q.next_idx();
        assert_eq!(n2, 8);

        let arr2 = q.pop_front_n::<10>();
        assert_eq!(arr2, None);
        let arr3 = q.pop_front_n::<4>();
        assert_eq!(arr3, Some([9, 15, 20, 25]));

        let n3 = q.next_idx();
        assert_eq!(n3, 8);

        assert!(q.is_empty());
        let b2 = q.base_idx();
        assert_eq!(b2, 8);
    }
}
