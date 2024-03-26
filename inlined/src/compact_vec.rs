use std::{
    fmt,
    hash::Hash,
    mem::MaybeUninit,
    ops::{Deref, DerefMut},
};

use super::{tiny_vec, TinyVec};

/// A contiguous array of elements. Similar to [`Vec<T>`], but can store up to a constant `N`
/// amount of elements inline before spilling over and allocating on the heap.
///
/// `N` should be strictly lower than 256.
#[derive(Clone)]
pub enum CompactVec<const N: usize, T> {
    Inlined(TinyVec<N, T>),
    Spilled(Vec<T>),
}

impl<const N: usize, T> Deref for CompactVec<N, T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        match self {
            Self::Inlined(tiny_vec) => tiny_vec.deref(),
            Self::Spilled(vec) => vec.deref(),
        }
    }
}

impl<const N: usize, T> DerefMut for CompactVec<N, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self {
            Self::Inlined(tiny_vec) => tiny_vec.deref_mut(),
            Self::Spilled(vec) => vec.deref_mut(),
        }
    }
}

impl<const N: usize, T: fmt::Debug> fmt::Debug for CompactVec<N, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.deref(), f)
    }
}

impl<const N: usize, T> Default for CompactVec<N, T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize, T: PartialEq> PartialEq for CompactVec<N, T> {
    fn eq(&self, other: &Self) -> bool {
        self.deref().eq(other.deref())
    }
}

impl<const N: usize, T: Eq> Eq for CompactVec<N, T> {}

impl<const N: usize, T: PartialOrd> PartialOrd for CompactVec<N, T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.deref().partial_cmp(other.deref())
    }
}

impl<const N: usize, T: Ord> Ord for CompactVec<N, T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.deref().cmp(other.deref())
    }
}

impl<const N: usize, T: Hash> Hash for CompactVec<N, T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.deref().hash(state)
    }
}

impl<const N: usize, T> CompactVec<N, T> {
    /// Constructs a new, empty `CompactVec`.
    pub const fn new() -> Self {
        Self::Inlined(TinyVec::new())
    }

    /// Returns whether this `CompactVec` has spilled over into the heap.
    pub fn is_spilled(&self) -> bool {
        matches!(self, Self::Spilled(_))
    }

    /// Returns the number of elements in this `CompactVec`.
    pub fn len(&self) -> usize {
        match self {
            Self::Inlined(tiny_vec) => tiny_vec.len() as usize,
            Self::Spilled(vec) => vec.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        match self {
            Self::Inlined(tiny_vec) => tiny_vec.is_empty(),
            Self::Spilled(vec) => vec.is_empty(),
        }
    }

    /// Returns the curernt capacity of this `CompactVec`.
    ///
    /// If the vector is inlined, this is the same as `N` clamped to 255. If this vector is
    /// spilled, then this is the capacity of the underlying `Vec` instance.
    pub fn capacity(&self) -> usize {
        match self {
            Self::Inlined(tiny_vec) => tiny_vec.capacity() as usize,
            Self::Spilled(vec) => vec.capacity(),
        }
    }

    /// Returns a slice containing the elements of this `CompactVec`.
    pub fn as_slice(&self) -> &[T] {
        self
    }

    /// Returns a mutable slice containing the elements of this `CompactVec`.
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        self
    }

    /// If spilled, returns `Some` with a reference to this `CompactVec`'s internal `Vec<T>`
    /// instance. Otherwise returns `None`.
    pub fn get_vec_if_spilled(&self) -> Option<&Vec<T>> {
        match self {
            Self::Inlined(_) => None,
            Self::Spilled(vec) => Some(vec),
        }
    }

    /// If spilled, returns `Some` with a mutable reference to this `CompactVec`'s internal
    /// `Vec<T>` instance. Otherwise returns `None`.
    pub fn get_vec_if_spilled_mut(&mut self) -> Option<&mut Vec<T>> {
        match self {
            Self::Inlined(_) => None,
            Self::Spilled(vec) => Some(vec),
        }
    }

    /// Ensures this `CompactVec` is spilled onto the heap by spilling it if it's not. Returns a
    /// mutable reference to the internal `Vec<T>` instance.
    ///
    /// If this `CompactVec` is already spilled, this call does nothing.
    pub fn spill(&mut self) -> &mut Vec<T> {
        self.spill_with_additional_capacity(0)
    }

    /// Ensures this `CompactVec` is spilled onto the heap by spilling it if it's not, while also
    /// ensuring the internal `Vec<T>` has the capacity to store the current elements, plus an
    /// additional `additional_length` elements.
    ///
    /// Returns a mutable reference to the internal `Vec<T>` instance.
    ///
    /// If this `CompactVec` is already spilled, this call does nothing.
    pub fn spill_with_additional_capacity(&mut self, additional_length: usize) -> &mut Vec<T> {
        let tiny_vec = match self {
            Self::Spilled(vec) => {
                vec.reserve(additional_length);
                return vec;
            }
            Self::Inlined(tiny_vec) => tiny_vec,
        };

        let capacity = additional_length
            .checked_add(tiny_vec.len() as usize)
            .expect("Capacity overflows usize");

        let mut vec = Vec::with_capacity(capacity);

        unsafe {
            let buf = tiny_vec.inner_buffer_mut();
            for i in 0..buf.len() {
                let ele = std::mem::replace(buf.get_unchecked_mut(i), MaybeUninit::uninit());
                vec.push(ele.assume_init());
            }
        }

        *self = Self::Spilled(vec);
        match self {
            Self::Spilled(vec) => vec,
            _ => unsafe { core::hint::unreachable_unchecked() },
        }
    }

    /// Appends an element at the end of this `CompactVec`.
    pub fn push(&mut self, element: T) {
        match self {
            Self::Inlined(tiny_vec) => {
                if let Some(element) = tiny_vec.push(element) {
                    self.spill_with_additional_capacity(1).push(element);
                }
            }
            Self::Spilled(vec) => vec.push(element),
        }
    }

    /// Inserts an element at position `index` within the `CompactVec`, shifting all elements after
    /// it to the right.
    ///
    /// # Panics
    ///
    /// Panics if `index > len`.
    pub fn insert(&mut self, index: usize, element: T) {
        match self {
            Self::Inlined(tiny_vec) => {
                if index > tiny_vec.len() as usize {
                    panic!("insertion index (is {index}) should be <= len (is {})", tiny_vec.len())
                }

                if let Some(element) = tiny_vec.insert(index as u8, element) {
                    self.spill_with_additional_capacity(1).insert(index, element);
                }
            }
            Self::Spilled(vec) => vec.insert(index, element),
        }
    }

    /// Removes the last element from this `CompactVec` and returns [`Some`] with it, or [`None`]
    /// if the vector was empty.
    pub fn pop(&mut self) -> Option<T> {
        match self {
            Self::Inlined(tiny_vec) => tiny_vec.pop(),
            Self::Spilled(vec) => vec.pop(),
        }
    }

    /// Removes and returns the element at position `index` within the `CompactVec`, shifting all
    /// elements after it to the left.
    ///
    /// Note: Because this shifts over the remaining elements, it has a worst-case performance of
    /// *O*(*n*). If you don't need the order of elements to be preserved, use
    /// [`swap_remove`](CompactVec::swap_remove) instead.
    ///
    /// # Panics
    ///
    /// Panics if `index` is out of bounds.
    pub fn remove(&mut self, index: usize) -> T {
        match self {
            Self::Inlined(tiny_vec) => {
                if index >= tiny_vec.len() as usize {
                    panic!("removal index (is {index}) should be < len (is {})", tiny_vec.len());
                }

                tiny_vec.remove(index as u8)
            }
            Self::Spilled(vec) => vec.remove(index),
        }
    }

    /// Removes and returns the element at position `index` within the `CompactVec`, replacing it
    /// with the last element of the vector.
    ///
    /// This does not preserve ordering of the remaining elements, but is *O*(1). If you need to
    /// preserve the element order, use [`remove`](CompactVec::remove) instead.
    ///
    /// # Panics
    ///
    /// Panics if `index` is out of bounds.
    pub fn swap_remove(&mut self, index: usize) -> T {
        match self {
            Self::Inlined(tiny_vec) => {
                if index >= tiny_vec.len() as usize {
                    panic!("swap_remove index (is {index}) should be < len (is {})", tiny_vec.len());
                }

                tiny_vec.swap_remove(index as u8)
            }
            Self::Spilled(vec) => vec.swap_remove(index),
        }
    }

    /// Clears this `CompactVec`, removing all values.
    pub fn clear(&mut self) {
        match self {
            Self::Inlined(tiny_vec) => tiny_vec.clear(),
            Self::Spilled(vec) => vec.clear(),
        }
    }

    /// Shortens this `CompactVec`, keeping the first `new_len` elements and dropping the rest.
    ///
    /// If `new_len` is greater or equal to the vector's current length, this has no effect.
    pub fn truncate(&mut self, new_len: usize) {
        match self {
            Self::Inlined(tiny_vec) if new_len <= u8::MAX as usize => tiny_vec.truncate(new_len as u8),
            Self::Spilled(vec) => vec.truncate(new_len),
            _ => {}
        }
    }
}

impl<const N: usize, T> Extend<T> for CompactVec<N, T> {
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        for ele in iter {
            self.push(ele);
        }
    }
}

impl<const N: usize, T> IntoIterator for CompactVec<N, T> {
    type Item = T;
    type IntoIter = IntoIter<N, T>;

    fn into_iter(self) -> Self::IntoIter {
        match self {
            Self::Inlined(tiny_vec) => IntoIter::Inlined(tiny_vec.into_iter()),
            Self::Spilled(vec) => IntoIter::Spilled(vec.into_iter()),
        }
    }
}

pub enum IntoIter<const N: usize, T> {
    Inlined(tiny_vec::IntoIter<N, T>),
    Spilled(std::vec::IntoIter<T>),
}

impl<const N: usize, T> Iterator for IntoIter<N, T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Inlined(iter) => iter.next(),
            Self::Spilled(iter) => iter.next(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::CompactVec;

    #[test]
    fn test_push_pop() {
        let mut vec = CompactVec::<3, char>::new();

        assert!(!vec.is_spilled());
        assert_eq!(vec.len(), 0);
        assert_eq!(vec.as_slice(), &[]);

        vec.push('a');
        assert!(!vec.is_spilled());
        assert_eq!(vec.len(), 1);
        assert_eq!(vec.as_slice(), &['a']);

        vec.push('b');
        assert!(!vec.is_spilled());
        assert_eq!(vec.len(), 2);
        assert_eq!(vec.as_slice(), &['a', 'b']);

        vec.push('c');
        assert!(!vec.is_spilled());
        assert_eq!(vec.len(), 3);
        assert_eq!(vec.as_slice(), &['a', 'b', 'c']);

        assert_eq!(vec.pop(), Some('c'));
        assert!(!vec.is_spilled());
        assert_eq!(vec.len(), 2);
        assert_eq!(vec.as_slice(), &['a', 'b']);

        vec.push('d');
        assert!(!vec.is_spilled());
        assert_eq!(vec.len(), 3);
        assert_eq!(vec.as_slice(), &['a', 'b', 'd']);

        vec.push('e');
        assert!(vec.is_spilled());
        assert_eq!(vec.len(), 4);
        assert_eq!(vec.as_slice(), &['a', 'b', 'd', 'e']);

        vec.extend(['x', 'y', 'z']);
        assert!(vec.is_spilled());
        assert_eq!(vec.len(), 7);
        assert_eq!(vec.as_slice(), &['a', 'b', 'd', 'e', 'x', 'y', 'z']);

        vec.truncate(4);
        assert!(vec.is_spilled());
        assert_eq!(vec.len(), 4);
        assert_eq!(vec.as_slice(), &['a', 'b', 'd', 'e']);

        assert_eq!(vec.pop(), Some('e'));
        assert!(vec.is_spilled());
        assert_eq!(vec.len(), 3);
        assert_eq!(vec.as_slice(), &['a', 'b', 'd']);

        assert_eq!(vec.pop(), Some('d'));
        assert!(vec.is_spilled());
        assert_eq!(vec.len(), 2);
        assert_eq!(vec.as_slice(), &['a', 'b']);

        vec.clear();
        assert!(vec.is_spilled());
        assert_eq!(vec.len(), 0);
        assert_eq!(vec.as_slice(), &[]);
    }

    #[test]
    fn test_insert_remove() {
        let mut vec = CompactVec::<3, char>::new();

        vec.insert(0, 'a');
        assert!(!vec.is_spilled());
        assert_eq!(vec.len(), 1);
        assert_eq!(vec.as_slice(), &['a']);

        vec.insert(0, 'b');
        assert!(!vec.is_spilled());
        assert_eq!(vec.len(), 2);
        assert_eq!(vec.as_slice(), &['b', 'a']);

        vec.insert(2, 'c');
        assert!(!vec.is_spilled());
        assert_eq!(vec.len(), 3);
        assert_eq!(vec.as_slice(), &['b', 'a', 'c']);

        vec.insert(1, 'd');
        assert!(vec.is_spilled());
        assert_eq!(vec.len(), 4);
        assert_eq!(vec.as_slice(), &['b', 'd', 'a', 'c']);

        assert_eq!(vec.remove(2), 'a');
        assert!(vec.is_spilled());
        assert_eq!(vec.len(), 3);
        assert_eq!(vec.as_slice(), &['b', 'd', 'c']);

        assert_eq!(vec.swap_remove(0), 'b');
        assert!(vec.is_spilled());
        assert_eq!(vec.len(), 2);
        assert_eq!(vec.as_slice(), &['c', 'd']);

        assert_eq!(vec.into_iter().collect::<Vec<_>>(), vec!['c', 'd']);
    }
}
