use std::{
    fmt,
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
        unsafe { std::mem::transmute(&self.inner[..self.len]) }
    }
}

impl<const N: usize, T> DerefMut for InlineVec<N, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { std::mem::transmute(&mut self.inner[..self.len]) }
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
            inner[i] = unsafe { MaybeUninit::new(self.inner[i].assume_init_ref().clone()) };
        }

        Self { inner, len: self.len }
    }
}

//impl<const N: usize, T: Copy> Copy for InlineVec<N, T> {}

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
    /// Returns `None` if the element was appended, or `Some` with the passed
    /// element if the vector is full.
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
                self.inner[index] = MaybeUninit::new(element);
            }
            self.len += 1;
            None
        }
    }

    pub fn pop(&mut self) -> Option<T> {
        if self.len == 0 {
            None
        } else {
            self.len -= 1;
            let retval = std::mem::replace(&mut self.inner[self.len], MaybeUninit::uninit());
            Some(unsafe { retval.assume_init() })
        }
    }

    pub fn remove(&mut self, index: usize) -> T {
        if index >= self.len {
            panic!("removal index (is {index}) should be < len (is {})", self.len);
        }

        let retval = std::mem::replace(&mut self.inner[index], MaybeUninit::uninit());
        self.len -= 1;

        unsafe {
            std::ptr::copy(
                self.inner.get_unchecked_mut(index + 1).as_ptr(),
                self.inner.get_unchecked_mut(index).as_mut_ptr(),
                self.len - index,
            );
            retval.assume_init()
        }
    }
}

impl<const N: usize, T> Drop for InlineVec<N, T> {
    fn drop(&mut self) {
        for i in 0..self.len {
            unsafe { self.inner[i].assume_init_drop() };
        }
    }
}
