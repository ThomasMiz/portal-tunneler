use core::fmt;
use std::ops::{Deref, DerefMut};

use super::InlineVec;

/// A UTF-8–encoded, inline string. Similar to [`String`], but stores chars inline instead of
/// allocating on the heap.
///
/// This means this "string" cannot store more than the constant `N` characters, and whether full
/// or empty will always occupy as much memory as if it were full. The upside to this is that this
/// memory is stored inline, so operations where a small string is needed can be optimized with
/// this type to make use of the stack, avoiding memory allocations and improving cache hits.
#[derive(Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InlineString<const N: usize> {
    inner: InlineVec<N, u8>,
}

impl<const N: usize> Deref for InlineString<N> {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl<const N: usize> DerefMut for InlineString<N> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { std::str::from_utf8_unchecked_mut(&mut self.inner) }
    }
}

impl<const N: usize> fmt::Debug for InlineString<N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.deref(), f)
    }
}

impl<const N: usize> fmt::Display for InlineString<N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self.deref(), f)
    }
}

impl<const N: usize> InlineString<N> {
    /// Creates a new empty `InlineString`.
    pub const fn new() -> Self {
        Self { inner: InlineVec::new() }
    }

    /// Returns the length of this `InlineString`, in bytes.
    pub const fn len(&self) -> usize {
        self.inner.len()
    }

    /// Returns this `InlineString`'s capacity, in bytes. This is the same as `N`.
    pub const fn capacity(&self) -> usize {
        self.inner.capacity()
    }

    pub fn as_str(&self) -> &str {
        unsafe { std::str::from_utf8_unchecked(self.inner.as_slice()) }
    }

    /// Appends a given string slice onto the end of this `InlineString`, returning how many bytes
    /// were appended.
    ///
    /// This function will attempt to copy as many characters as possible, but will respect UTF-8
    /// char boundaries. This means that if your `InlineString` has only one remaining byte of
    /// capacity and you try to push an 'á' character (whose size is 2 bytes), nothing will occur.
    pub fn push_str(&mut self, string: &str) -> usize {
        let remaining_capacity = self.capacity() - self.len();

        let byte_count = match remaining_capacity >= string.len() {
            true => string.len(),
            false => string.floor_char_boundary(remaining_capacity),
        };

        unsafe { self.inner.extend_from_slice_copied(string.as_bytes().get_unchecked(0..byte_count)) };
        byte_count
    }

    /// Appends the given [`char`] to the end of this `InlineString`, returning how many bytes were
    /// appended.
    ///
    /// This function will respect UTF-8 char boundaries, so if your `InlineString` has only one
    /// remaining byte of capacity and you try to push an 'á' character (whose size is 2 bytes),
    /// nothing will occur.
    pub fn push(&mut self, ch: char) -> usize {
        let utf8_len = ch.len_utf8();
        if utf8_len > self.capacity() - self.len() {
            return 0;
        }

        match utf8_len {
            1 => {
                let _ = self.inner.push(ch as u8);
                1
            }
            _ => self.inner.extend_from_slice(ch.encode_utf8(&mut [0; 4]).as_bytes()),
        }
    }

    /// Removes the last character from this `InlineString` and returns [`Some`] with it, or
    /// [`None`] if the string was empty.
    pub fn pop(&mut self) -> Option<char> {
        let (new_len, ch) = self.char_indices().next_back()?;
        unsafe { self.inner.set_len(new_len) };
        Some(ch)
    }

    /// Clears this `InlineString`, removing all contents.
    pub fn clear(&mut self) {
        self.inner.clear();
    }

    /// Shortens this `InlineString` to the specified length.
    ///
    /// # Panics
    ///
    /// Panics if `new_len` does not lie on a [`char`] boundary.
    pub fn truncate(&mut self, new_len: usize) {
        if new_len < self.len() {
            if !self.is_char_boundary(new_len) {
                panic!("new_len does not lie on a char boundary");
            }

            self.inner.truncate(new_len);
        }
    }
}

impl<const N: usize> std::fmt::Write for InlineString<N> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.push_str(s);
        Ok(())
    }

    fn write_char(&mut self, c: char) -> fmt::Result {
        self.push(c);
        Ok(())
    }
}

impl<const N: usize> From<&str> for InlineString<N> {
    fn from(value: &str) -> Self {
        let mut s = InlineString::new();
        s.push_str(value);
        s
    }
}

#[cfg(test)]
mod tests {
    use std::{fmt::Write, ops::Deref};

    use super::InlineString;

    #[test]
    fn test_push_pop() {
        let mut s = InlineString::<5>::new();

        assert_eq!(s.push('a'), 1);
        assert_eq!(s.len(), 1);
        assert_eq!(s.deref(), "a");

        assert_eq!(s.push('á'), 2);
        assert_eq!(s.len(), 3);
        assert_eq!(s.deref(), "aá");

        assert_eq!(s.push('b'), 1);
        assert_eq!(s.len(), 4);
        assert_eq!(s.deref(), "aáb");

        assert_eq!(s.push('ó'), 0);
        assert_eq!(s.len(), 4);
        assert_eq!(s.deref(), "aáb");

        assert_eq!(s.push_str("ó"), 0);
        assert_eq!(s.len(), 4);
        assert_eq!(s.deref(), "aáb");

        assert_eq!(s.push_str("o"), 1);
        assert_eq!(s.len(), 5);
        assert_eq!(s.deref(), "aábo");

        assert_eq!(s.push('i'), 0);
        assert_eq!(s.len(), 5);
        assert_eq!(s.deref(), "aábo");

        assert_eq!(s.push_str("please and thank you"), 0);
        assert_eq!(s.len(), 5);
        assert_eq!(s.deref(), "aábo");

        assert_eq!(s.pop(), Some('o'));
        assert_eq!(s.len(), 4);
        assert_eq!(s.deref(), "aáb");

        assert_eq!(s.pop(), Some('b'));
        assert_eq!(s.len(), 3);
        assert_eq!(s.deref(), "aá");

        assert_eq!(s.push('û'), 2);
        assert_eq!(s.len(), 5);
        assert_eq!(s.deref(), "aáû");

        assert_eq!(s.push('û'), 0);
        assert_eq!(s.len(), 5);
        assert_eq!(s.deref(), "aáû");

        assert_eq!(s.push_str("û"), 0);
        assert_eq!(s.len(), 5);
        assert_eq!(s.deref(), "aáû");

        assert_eq!(s.pop(), Some('û'));
        assert_eq!(s.len(), 3);
        assert_eq!(s.deref(), "aá");

        assert_eq!(s.push_str("êxe"), 2);
        assert_eq!(s.len(), 5);
        assert_eq!(s.deref(), "aáê");

        assert_eq!(s.pop(), Some('ê'));
        assert_eq!(s.len(), 3);
        assert_eq!(s.deref(), "aá");

        assert_eq!(s.push_str("WHAT"), 2);
        assert_eq!(s.len(), 5);
        assert_eq!(s.deref(), "aáWH");

        assert_eq!(s.pop(), Some('H'));
        assert_eq!(s.len(), 4);
        assert_eq!(s.deref(), "aáW");

        assert_eq!(s.pop(), Some('W'));
        assert_eq!(s.len(), 3);
        assert_eq!(s.deref(), "aá");

        assert_eq!(s.pop(), Some('á'));
        assert_eq!(s.len(), 1);
        assert_eq!(s.deref(), "a");

        assert_eq!(s.pop(), Some('a'));
        assert_eq!(s.len(), 0);
        assert_eq!(s.deref(), "");
    }

    #[test]
    fn test_push_str_limit() {
        let mut s = InlineString::<5>::new();

        assert_eq!(s.push_str("ÁRTICO"), 5);
        assert_eq!(s.len(), 5);
        assert_eq!(s.deref(), "ÁRTI");

        s.clear();
        assert_eq!(s.len(), 0);
        assert_eq!(s.deref(), "");

        assert_eq!(s.push_str("ANTÁRTICO"), 5);
        assert_eq!(s.len(), 5);
        assert_eq!(s.deref(), "ANTÁ");

        s.truncate(0);
        assert_eq!(s.len(), 0);
        assert_eq!(s.deref(), "");

        assert_eq!(s.push_str("CUCHÁ"), 4);
        assert_eq!(s.len(), 4);
        assert_eq!(s.deref(), "CUCH");
    }

    #[test]
    fn test_clear() {
        let mut s = InlineString::<10>::from("crocante");

        s.clear();
        assert_eq!(s.len(), 0);
        assert_eq!(s.deref(), "");
    }

    #[test]
    fn test_truncate() {
        let mut s = InlineString::<16>::new();

        assert_eq!(s.len(), 0);
        assert_eq!(s.deref(), "");

        assert_eq!(s.push_str("á!éxíd&ó"), 12);
        assert_eq!(s.len(), 12);
        assert_eq!(s.deref(), "á!éxíd&ó");

        s.truncate(12);
        assert_eq!(s.len(), 12);
        assert_eq!(s.deref(), "á!éxíd&ó");

        s.truncate(10);
        assert_eq!(s.len(), 10);
        assert_eq!(s.deref(), "á!éxíd&");

        s.truncate(250);
        assert_eq!(s.len(), 10);
        assert_eq!(s.deref(), "á!éxíd&");

        s.truncate(9);
        assert_eq!(s.len(), 9);
        assert_eq!(s.deref(), "á!éxíd");

        s.truncate(2);
        assert_eq!(s.len(), 2);
        assert_eq!(s.deref(), "á");

        s.truncate(0);
        assert_eq!(s.len(), 0);
        assert_eq!(s.deref(), "");
    }

    #[test]
    #[should_panic]
    fn test_truncate_boundary_panics() {
        InlineString::<8>::from("ü").truncate(1);
    }

    #[test]
    fn test_write() {
        let mut s = InlineString::<10>::new();

        assert_eq!(write!(s, "Hello: {}", 123), Ok(()));
        assert_eq!(s.deref(), "Hello: 123");

        s.clear();
        assert_eq!(write!(s, "Hello"), Ok(()));
        assert_eq!(write!(s, ": "), Ok(()));
        assert_eq!(write!(s, "{}", 123), Ok(()));
        assert_eq!(s.deref(), "Hello: 123");

        s.clear();
        assert_eq!(write!(s, "Goodbye"), Ok(()));
        assert_eq!(write!(s, ": "), Ok(()));
        assert_eq!(write!(s, "{}", 123), Ok(()));
        assert_eq!(s.deref(), "Goodbye: 1");
    }
}
