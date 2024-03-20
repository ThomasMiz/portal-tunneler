use std::{
    fmt,
    hash::Hash,
    mem::MaybeUninit,
    ops::{Deref, DerefMut},
};

pub struct InlineVec<const N: usize, T> {
    inner: [MaybeUninit<T>; N],
    len: usize,
}

impl<const N: usize, T> Deref for InlineVec<N, T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        unsafe { std::mem::transmute(self.inner.get_unchecked(..self.len)) }
    }
}

impl<const N: usize, T> DerefMut for InlineVec<N, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { std::mem::transmute(self.inner.get_unchecked_mut(..self.len)) }
    }
}

impl<const N: usize, T: fmt::Debug> fmt::Debug for InlineVec<N, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        fmt::Debug::fmt(self.deref(), f)
    }
}

impl<const N: usize, T: Clone> Clone for InlineVec<N, T> {
    fn clone(&self) -> Self {
        let mut inner: [MaybeUninit<T>; N] = unsafe { MaybeUninit::uninit().assume_init() };
        for i in 0..self.len {
            unsafe { *inner.get_unchecked_mut(i) = MaybeUninit::new(self.inner.get_unchecked(i).assume_init_ref().clone()) };
        }

        Self { inner, len: self.len }
    }
}

impl<const N: usize, T> Default for InlineVec<N, T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize, T: PartialEq> PartialEq for InlineVec<N, T> {
    fn eq(&self, other: &Self) -> bool {
        self.deref().eq(other.deref())
    }
}

impl<const N: usize, T: Eq> Eq for InlineVec<N, T> {}

impl<const N: usize, T: PartialOrd> PartialOrd for InlineVec<N, T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.deref().partial_cmp(other.deref())
    }
}

impl<const N: usize, T: Ord> Ord for InlineVec<N, T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.deref().cmp(other.deref())
    }
}

impl<const N: usize, T: Hash> Hash for InlineVec<N, T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.deref().hash(state)
    }
}

impl<const N: usize, T> InlineVec<N, T> {
    pub const fn new() -> Self {
        Self {
            inner: unsafe { MaybeUninit::uninit().assume_init() },
            len: 0,
        }
    }

    /// Returns the number of elements in this vector.
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Returns the maximum capacity of this vector. This is the same as for `N`.
    pub const fn capacity(&self) -> usize {
        N
    }

    /// Appends an element at the end of this vector.
    ///
    /// Returns [`None`] if the element was appended, or [`Some`] with the passed element if the
    /// vector is full.
    pub fn push(&mut self, element: T) -> Option<T> {
        if self.len == self.inner.len() {
            Some(element)
        } else {
            unsafe {
                *self.inner.get_unchecked_mut(self.len) = MaybeUninit::new(element);
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
    pub fn insert(&mut self, index: usize, element: T) -> Option<T> {
        if index > self.len {
            panic!("insertion index (is {index}) should be <= len (is {})", self.len)
        }

        if self.len == self.inner.len() {
            Some(element)
        } else {
            unsafe {
                std::ptr::copy(
                    self.inner.get_unchecked_mut(index).as_ptr(),
                    self.inner.get_unchecked_mut(index + 1).as_mut_ptr(),
                    self.len - index,
                );
                *self.inner.get_unchecked_mut(index) = MaybeUninit::new(element);
            }
            self.len += 1;
            None
        }
    }

    /// Removes the last element from a vector and returns it, or [`None`] if it is empty.
    pub fn pop(&mut self) -> Option<T> {
        if self.len == 0 {
            None
        } else {
            unsafe {
                self.len -= 1;
                let retval = std::mem::replace(self.inner.get_unchecked_mut(self.len), MaybeUninit::uninit());
                Some(retval.assume_init())
            }
        }
    }

    /// Removes and returns the element at position `index` within the vector, shifting all
    /// elements after it to the left.
    ///
    /// Note: Because this shifts over the remaining elements, it has a worst-case performance of
    /// *O*(*n*). If you don't need the order of elements to be preserved, use [`swap_remove`]
    /// instead.
    ///
    /// [`swap_remove`]: InlineVec::swap_remove
    ///
    /// # Panics
    ///
    /// Panics if `index` is out of bounds.
    pub fn remove(&mut self, index: usize) -> T {
        if index >= self.len {
            panic!("removal index (is {index}) should be < len (is {})", self.len);
        }

        unsafe {
            let retval = std::mem::replace(self.inner.get_unchecked_mut(index), MaybeUninit::uninit());
            self.len -= 1;

            std::ptr::copy(
                self.inner.get_unchecked_mut(index + 1).as_ptr(),
                self.inner.get_unchecked_mut(index).as_mut_ptr(),
                self.len - index,
            );
            retval.assume_init()
        }
    }

    /// Removes an element from the vector and returns it.
    ///
    /// The removed element is replaced by the last element of the vector.
    ///
    /// This does not preserve ordering of the remaining elements, but is *O*(1). If you need to
    /// preserve the element order, use [`remove`] instead.
    ///
    /// [`remove`]: InlineVec::remove
    ///
    /// # Panics
    ///
    /// Panics if `index` is out of bounds.
    pub fn swap_remove(&mut self, index: usize) -> T {
        if index >= self.len {
            panic!("swap_remove index (is {index}) should be < len (is {})", self.len);
        }

        unsafe {
            let retval = std::mem::replace(self.inner.get_unchecked_mut(index), MaybeUninit::uninit());
            self.len -= 1;

            if self.len != index {
                *self.inner.get_unchecked_mut(index) = std::mem::transmute_copy(self.inner.get_unchecked_mut(self.len));
            }

            retval.assume_init()
        }
    }
}

impl<const N: usize, T> Drop for InlineVec<N, T> {
    fn drop(&mut self) {
        for i in 0..self.len {
            unsafe { self.inner.get_unchecked_mut(i).assume_init_drop() };
        }
    }
}

impl<const N: usize, T> IntoIterator for InlineVec<N, T> {
    type Item = T;
    type IntoIter = IntoIter<N, T>;

    fn into_iter(mut self) -> Self::IntoIter {
        let empty_inner = unsafe { MaybeUninit::uninit().assume_init() };

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
    len: usize,
    index: usize,
}

impl<const N: usize, T> Iterator for IntoIter<N, T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index == self.len {
            None
        } else {
            unsafe {
                let element = std::mem::replace(self.inner.get_unchecked_mut(self.index), MaybeUninit::uninit());
                self.index += 1;

                Some(element.assume_init())
            }
        }
    }
}

impl<const N: usize, T> Drop for IntoIter<N, T> {
    fn drop(&mut self) {
        for i in self.index..self.len {
            unsafe { self.inner.get_unchecked_mut(i).assume_init_drop() };
        }
    }
}
