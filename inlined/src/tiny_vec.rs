use std::{
    fmt,
    hash::Hash,
    io,
    mem::MaybeUninit,
    ops::{Deref, DerefMut},
};

/// A contiguous array of elements. Similar to [`Vec<T>`], but stores elements inline instead of
/// allocating on the heap. Similar to [`InlineVec`](super::InlineVec), but has an `u8` length
/// instead of `usize`, and thus cannot have a capacity greater than 255.
///
/// This means this "vector" cannot store more than the constant `N` elements, and whether full or
/// empty will always occupy as much memory as if it were full. The upside to this is that this
/// memory is stored inline, so operations where a small vector is needed can be optimized with
/// this type to make use of the stack, avoiding memory allocations and improving cache hits.
///
/// `N` should be strictly lower than 256.
pub struct TinyVec<const N: usize, T> {
    len: u8,
    inner: [MaybeUninit<T>; N],
}

impl<const N: usize, T> Deref for TinyVec<N, T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        unsafe { std::mem::transmute(self.inner.get_unchecked(..(self.len as usize))) }
    }
}

impl<const N: usize, T> DerefMut for TinyVec<N, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { std::mem::transmute(self.inner.get_unchecked_mut(..(self.len as usize))) }
    }
}

impl<const N: usize, T: fmt::Debug> fmt::Debug for TinyVec<N, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        fmt::Debug::fmt(self.deref(), f)
    }
}

impl<const N: usize, T: Clone> Clone for TinyVec<N, T> {
    fn clone(&self) -> Self {
        let mut inner: [MaybeUninit<T>; N] = unsafe { MaybeUninit::uninit().assume_init() };
        for i in 0..(self.len as usize) {
            unsafe { *inner.get_unchecked_mut(i) = MaybeUninit::new(self.inner.get_unchecked(i).assume_init_ref().clone()) };
        }

        Self { inner, len: self.len }
    }
}

impl<const N: usize, T> Default for TinyVec<N, T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize, T: PartialEq> PartialEq for TinyVec<N, T> {
    fn eq(&self, other: &Self) -> bool {
        self.deref().eq(other.deref())
    }
}

impl<const N: usize, T: Eq> Eq for TinyVec<N, T> {}

impl<const N: usize, T: PartialOrd> PartialOrd for TinyVec<N, T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.deref().partial_cmp(other.deref())
    }
}

impl<const N: usize, T: Ord> Ord for TinyVec<N, T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.deref().cmp(other.deref())
    }
}

impl<const N: usize, T: Hash> Hash for TinyVec<N, T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.deref().hash(state)
    }
}

impl<const N: usize, T> TinyVec<N, T> {
    /// Constructs a new, empty `TinyVec`.
    pub const fn new() -> Self {
        Self {
            inner: unsafe { MaybeUninit::uninit().assume_init() },
            len: 0,
        }
    }

    /// Returns the number of elements in this vector.
    pub const fn len(&self) -> u8 {
        self.len
    }

    /// Returns `true` if this `InlineVec` contains no elements, and `false` otherwise.
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns the maximum capacity of this vector. This is `N` clamped up to 255.
    pub const fn capacity(&self) -> u8 {
        if N > u8::MAX as usize {
            u8::MAX
        } else {
            N as u8
        }
    }

    /// Returns a slice over the elements of this `TinyVec`.
    pub fn as_slice(&self) -> &[T] {
        self
    }

    /// Returns a mutable slice over the elements of this `TinyVec`.
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        self
    }

    /// Appends an element at the end of this vector.
    ///
    /// Returns [`None`] if the element was appended, or [`Some`] with the passed element if the
    /// vector is full.
    pub fn push(&mut self, element: T) -> Option<T> {
        if self.len == self.capacity() {
            Some(element)
        } else {
            unsafe {
                *self.inner.get_unchecked_mut(self.len as usize) = MaybeUninit::new(element);
            }
            self.len += 1;
            None
        }
    }

    /// Inserts an element at position `index` within the vector, shifting all elements after it to
    /// the right.
    ///
    /// Returns [`None`] if the element was inserted, or [`Some`] with the passed element if the
    /// vector is full.
    ///
    /// # Panics
    ///
    /// Panics if `index > len`.
    pub fn insert(&mut self, index: u8, element: T) -> Option<T> {
        if index > self.len {
            panic!("insertion index (is {index}) should be <= len (is {})", self.len)
        }

        if self.len == self.capacity() {
            Some(element)
        } else {
            unsafe {
                if index != self.len {
                    let ptr = self.inner.as_mut_ptr().add(index as usize);
                    std::ptr::copy(ptr, ptr.add(1), (self.len - index) as usize);
                }
                *self.inner.get_unchecked_mut(index as usize) = MaybeUninit::new(element);
            }
            self.len += 1;
            None
        }
    }

    /// Removes the last element from this `TinyVec` and returns [`Some`] with it, or [`None`] if
    /// the vector was empty.
    pub fn pop(&mut self) -> Option<T> {
        if self.len == 0 {
            None
        } else {
            unsafe {
                self.len -= 1;
                let retval = std::mem::replace(self.inner.get_unchecked_mut(self.len as usize), MaybeUninit::uninit());
                Some(retval.assume_init())
            }
        }
    }

    /// Removes and returns the element at position `index` within the vector, shifting all
    /// elements after it to the left.
    ///
    /// Note: Because this shifts over the remaining elements, it has a worst-case performance of
    /// *O*(*n*). If you don't need the order of elements to be preserved, use
    /// [`swap_remove`](TinyVec::swap_remove) instead.
    ///
    /// # Panics
    ///
    /// Panics if `index` is out of bounds.
    pub fn remove(&mut self, index: u8) -> T {
        if index >= self.len {
            panic!("removal index (is {index}) should be < len (is {})", self.len);
        }

        unsafe {
            let retval = std::mem::replace(self.inner.get_unchecked_mut(index as usize), MaybeUninit::uninit());
            self.len -= 1;

            if index != self.len {
                let ptr = self.inner.as_mut_ptr().add(index as usize);
                std::ptr::copy(ptr.add(1), ptr, (self.len - index) as usize);
            }
            retval.assume_init()
        }
    }

    /// Removes an element from the vector and returns it.
    ///
    /// The removed element is replaced by the last element of the vector.
    ///
    /// This does not preserve ordering of the remaining elements, but is *O*(1). If you need to
    /// preserve the element order, use [`remove`](TinyVec::remove) instead.
    ///
    /// # Panics
    ///
    /// Panics if `index` is out of bounds.
    pub fn swap_remove(&mut self, index: u8) -> T {
        if index >= self.len {
            panic!("swap_remove index (is {index}) should be < len (is {})", self.len);
        }

        unsafe {
            let retval = std::mem::replace(self.inner.get_unchecked_mut(index as usize), MaybeUninit::uninit());
            self.len -= 1;

            if self.len != index {
                *self.inner.get_unchecked_mut(index as usize) = std::mem::transmute_copy(self.inner.get_unchecked_mut(self.len as usize));
            }

            retval.assume_init()
        }
    }

    /// Clears this `TinyVec`, removing all values.
    pub fn clear(&mut self) {
        for i in 0..(self.len as usize) {
            unsafe { self.inner.get_unchecked_mut(i).assume_init_drop() };
        }

        self.len = 0;
    }

    /// Shortens the `TinyVec`, keeping the first `new_len` elements and dropping the rest.
    ///
    /// If `new_len` is greater or equal to the vector's current length, this has no effect.
    pub fn truncate(&mut self, new_len: u8) {
        if new_len < self.len {
            for i in new_len..self.len {
                unsafe { self.inner.get_unchecked_mut(i as usize).assume_init_drop() };
            }

            self.len = new_len;
        }
    }

    /// Gets a mutable reference to this `TinyVec`'s internal storage, which may be partly
    /// uninitialized. This operation is unsafe, and the caller is responsible for ensuring this
    /// type's invariants are maintaned.
    ///
    /// # Safety
    ///
    /// - If the length of the vector is modified, it should be set with [`set_len`](TinyVec::set_len)
    /// - There must be no uninitialized elements in the range 0..self.len()
    /// - Any elements removed must be manually dropped by the caller
    pub unsafe fn inner_buffer_mut(&mut self) -> &mut [MaybeUninit<T>; N] {
        &mut self.inner
    }

    /// Forces the length of the `TinyVec` to `new_len`. This operation is unsafe, and the caller
    /// is responsible for ensuring this type's invariants are maintaned.
    ///
    /// # Safety
    ///
    /// - `new_len` must be less than or equal to [`capacity`](TinyVec::capacity).
    /// - The elements in between the old and new lengths must be either initialized or dropped
    /// (depending on whether the vector is being expanded or truncated).
    pub unsafe fn set_len(&mut self, new_len: u8) {
        self.len = new_len;
    }
}

impl<const N: usize, T> Extend<T> for TinyVec<N, T> {
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        for ele in iter {
            if self.push(ele).is_some() {
                break;
            }
        }
    }
}

impl<const N: usize, T: Clone> TinyVec<N, T> {
    /// Clones and appends as many elements as possible from the slice to the `Vec`. Returns the
    /// amount of appended elements.
    pub fn extend_from_slice(&mut self, other: &[T]) -> u8 {
        let count = other.len().min((self.capacity() - self.len) as usize) as u8;

        if count != 0 {
            for i in 0..(count as usize) {
                unsafe {
                    *self.inner.get_unchecked_mut(self.len as usize + i) = MaybeUninit::new(other.get_unchecked(i).clone());
                }
            }

            self.len += count;
        }

        count
    }
}

impl<const N: usize, T: Copy> TinyVec<N, T> {
    /// Copies and appends as many elements as possible from the slice to the `Vec`. Returns the
    /// amount of appended elements.
    pub fn extend_from_slice_copied(&mut self, other: &[T]) -> u8 {
        let count = other.len().min((self.capacity() - self.len) as usize) as u8;

        if count != 0 {
            unsafe {
                let dst = std::mem::transmute(self.inner.as_mut_ptr().add(self.len as usize));
                std::ptr::copy_nonoverlapping(other.as_ptr(), dst, count as usize);
                self.len += count;
            }
        }

        count
    }
}

impl<const N: usize> io::Write for TinyVec<N, u8> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        Ok(self.extend_from_slice_copied(buf) as usize)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl<const N: usize, T> Drop for TinyVec<N, T> {
    fn drop(&mut self) {
        for i in 0..(self.len as usize) {
            unsafe { self.inner.get_unchecked_mut(i).assume_init_drop() };
        }
    }
}

impl<const N: usize, T> IntoIterator for TinyVec<N, T> {
    type Item = T;
    type IntoIter = IntoIter<N, T>;

    fn into_iter(mut self) -> Self::IntoIter {
        let empty_inner: [MaybeUninit<T>; N] = unsafe { MaybeUninit::uninit().assume_init() };

        let result = IntoIter {
            inner: std::mem::replace(&mut self.inner, empty_inner),
            len: self.len,
            index: 0,
        };

        std::mem::forget(self);
        result
    }
}

pub struct IntoIter<const N: usize, T> {
    inner: [MaybeUninit<T>; N],
    len: u8,
    index: u8,
}

impl<const N: usize, T> Iterator for IntoIter<N, T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index == self.len {
            None
        } else {
            unsafe {
                let element = std::mem::replace(self.inner.get_unchecked_mut(self.index as usize), MaybeUninit::uninit());
                self.index += 1;

                Some(element.assume_init())
            }
        }
    }
}

impl<const N: usize, T> Drop for IntoIter<N, T> {
    fn drop(&mut self) {
        for i in self.index..self.len {
            unsafe { self.inner.get_unchecked_mut(i as usize).assume_init_drop() };
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{io::Write, ops::Deref};

    use crate::test_utils::DropChecker;

    use super::TinyVec;

    #[test]
    fn test_push_pop() {
        let mut vec = TinyVec::<3, i32>::new();

        assert_eq!(vec.pop(), None);
        assert_eq!(vec.len(), 0);
        assert_eq!(vec.deref(), &[]);

        assert_eq!(vec.push(1), None);
        assert_eq!(vec.len(), 1);
        assert_eq!(vec.deref(), &[1]);
        assert_eq!(vec.push(2), None);
        assert_eq!(vec.len(), 2);
        assert_eq!(vec.deref(), &[1, 2]);
        assert_eq!(vec.push(3), None);
        assert_eq!(vec.len(), 3);
        assert_eq!(vec.deref(), &[1, 2, 3]);
        assert_eq!(vec.push(4), Some(4));
        assert_eq!(vec.len(), 3);
        assert_eq!(vec.deref(), &[1, 2, 3]);

        assert_eq!(vec.pop(), Some(3));
        assert_eq!(vec.len(), 2);
        assert_eq!(vec.deref(), &[1, 2]);
        assert_eq!(vec.pop(), Some(2));
        assert_eq!(vec.len(), 1);
        assert_eq!(vec.deref(), &[1]);
        assert_eq!(vec.pop(), Some(1));
        assert_eq!(vec.len(), 0);
        assert_eq!(vec.deref(), &[]);
        assert_eq!(vec.pop(), None);
        assert_eq!(vec.len(), 0);
        assert_eq!(vec.deref(), &[]);
    }

    #[test]
    fn test_insert_remove() {
        let mut vec = TinyVec::<3, _>::new();

        assert_eq!(vec.insert(0, 'a'), None);
        assert_eq!(vec.deref(), &['a']);
        assert_eq!(vec.insert(0, 'b'), None);
        assert_eq!(vec.deref(), &['b', 'a']);
        assert_eq!(vec.insert(1, 'c'), None);
        assert_eq!(vec.deref(), &['b', 'c', 'a']);

        for i in 0..vec.len() {
            assert_eq!(vec.insert(i, 'x'), Some('x'));
        }

        assert_eq!(vec.remove(0), 'b');
        assert_eq!(vec.remove(0), 'c');
        assert_eq!(vec.remove(0), 'a');
        assert_eq!(vec.len(), 0);

        assert_eq!(vec.insert(0, '0'), None);
        assert_eq!(vec.deref(), &['0']);
        assert_eq!(vec.insert(1, '1'), None);
        assert_eq!(vec.deref(), &['0', '1']);
        assert_eq!(vec.insert(2, '2'), None);
        assert_eq!(vec.deref(), &['0', '1', '2']);
        assert_eq!(vec.insert(3, '3'), Some('3'));
        assert_eq!(vec.deref(), &['0', '1', '2']);

        assert_eq!(vec.swap_remove(0), '0');
        assert_eq!(vec.deref(), &['2', '1']);
        assert_eq!(vec.swap_remove(1), '1');
        assert_eq!(vec.deref(), &['2']);
        assert_eq!(vec.push('1'), None);
        assert_eq!(vec.push('0'), None);
        assert_eq!(vec.deref(), &['2', '1', '0']);

        assert_eq!(vec.swap_remove(2), '0');
        assert_eq!(vec.deref(), &['2', '1']);
        assert_eq!(vec.swap_remove(0), '2');
        assert_eq!(vec.deref(), &['1']);
        assert_eq!(vec.swap_remove(0), '1');
        assert_eq!(vec.deref(), &[]);
    }

    #[test]
    #[should_panic]
    fn test_insert_panics0() {
        TinyVec::<3, i32>::new().insert(1, 5);
    }

    #[test]
    #[should_panic]
    fn test_insert_panics1() {
        let mut vec = TinyVec::<3, i32>::new();
        vec.push(69);
        vec.insert(2, 69);
    }

    #[test]
    #[should_panic]
    fn test_remove_panics0() {
        TinyVec::<3, i32>::new().remove(0);
    }

    #[test]
    #[should_panic]
    fn test_remove_panics1() {
        let mut vec = TinyVec::<3, i32>::new();
        vec.push(69);
        vec.remove(1);
    }

    #[test]
    #[should_panic]
    fn test_swap_remove_panics0() {
        TinyVec::<3, i32>::new().swap_remove(0);
    }

    #[test]
    #[should_panic]
    fn test_swap_remove_panics1() {
        let mut vec = TinyVec::<3, i32>::new();
        vec.push(69);
        vec.swap_remove(1);
    }

    #[test]
    fn test_drops() {
        let mut dc = DropChecker::new();
        let mut vec = TinyVec::<3, _>::new();

        assert_eq!(vec.push(dc.track(5)), None);
        assert_eq!(vec.push(dc.track(10)), None);
        assert_eq!(vec.pop().map(|s| s.value), Some(10));
        assert_eq!(vec.push(dc.track(69)), None);

        drop(vec);
        dc.ensure_all_dropped();
    }

    #[test]
    fn test_into_iter() {
        let mut dc = DropChecker::new();
        let mut vec = TinyVec::<3, _>::new();

        assert_eq!(vec.push(dc.track(10)), None);
        assert_eq!(vec.push(dc.track(20)), None);
        assert_eq!(vec.push(dc.track(30)), None);

        let mut iter = vec.into_iter();
        assert!(iter.next().is_some_and(|v| v.value == 10));
        assert!(iter.next().is_some_and(|v| v.value == 20));
        assert!(iter.next().is_some_and(|v| v.value == 30));
        assert_eq!(iter.next(), None);

        dc.ensure_all_dropped();

        let s = "jfmq29o8cut1o24t9movqj24";
        let mut vec = TinyVec::<24, _>::new();

        for ch in s.chars() {
            assert_eq!(vec.push(dc.track(String::from(ch))), None);
        }

        let mut iter = vec.into_iter();
        for i in 0..10 {
            let si = iter.next().unwrap();
            let ch = s.chars().nth(i).unwrap();
            assert_eq!(si.value, String::from(ch));
        }
        drop(iter);

        dc.ensure_all_dropped();
    }

    #[test]
    fn test_write() {
        let mut vec = TinyVec::<5, u8>::new();

        assert!(vec.write(&[4, 20]).is_ok_and(|v| v == 2));
        assert_eq!(vec.deref(), &[4, 20]);

        assert_eq!(vec.push(69), None);
        assert_eq!(vec.deref(), &[4, 20, 69]);

        assert!(vec.write(&[7, 8, 9, 10, 11, 12]).is_ok_and(|v| v == 2));
        assert_eq!(vec.deref(), &[4, 20, 69, 7, 8]);

        assert!(vec.write(&[50]).is_ok_and(|v| v == 0));
        assert_eq!(vec.deref(), &[4, 20, 69, 7, 8]);

        assert_eq!(vec.pop(), Some(8));
        assert_eq!(vec.deref(), &[4, 20, 69, 7]);

        assert!(vec.write(&[90, 91, 92, 93, 94, 95, 96]).is_ok_and(|v| v == 1));
        assert_eq!(vec.deref(), &[4, 20, 69, 7, 90]);

        assert!(vec.write(&[100, 101, 102, 103, 104]).is_ok_and(|v| v == 0));
        assert_eq!(vec.deref(), &[4, 20, 69, 7, 90]);
    }
}
