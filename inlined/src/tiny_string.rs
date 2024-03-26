use core::fmt;
use std::ops::{Deref, DerefMut};

use super::TinyVec;

/// A UTF-8–encoded, inline string. Similar to [`String`], but stores chars inline instead of
/// allocating on the heap. Similar to [`InlineString`](super::InlineString), but has an `u8`
/// length instead of `usize`, and thus cannot have a capacity greater than 255.
///
/// This means this "string" cannot store more than the constant `N` characters, and whether full
/// or empty will always occupy as much memory as if it were full. The upside to this is that this
/// memory is stored inline, so operations where a small string is needed can be optimized with
/// this type to make use of the stack, avoiding memory allocations and improving cache hits.
///
/// `N` should be strictly lower than 256.
#[derive(Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TinyString<const N: usize> {
    inner: TinyVec<N, u8>,
}

impl<const N: usize> Deref for TinyString<N> {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        unsafe { std::str::from_utf8_unchecked(self.inner.as_slice()) }
    }
}

impl<const N: usize> DerefMut for TinyString<N> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { std::str::from_utf8_unchecked_mut(&mut self.inner) }
    }
}

impl<const N: usize> fmt::Debug for TinyString<N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.deref(), f)
    }
}

impl<const N: usize> fmt::Display for TinyString<N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self.deref(), f)
    }
}

impl<const N: usize> TinyString<N> {
    /// Creates a new empty `TinyString`.
    pub const fn new() -> Self {
        Self { inner: TinyVec::new() }
    }

    /// Returns the length of this `TinyString`, in bytes.
    pub const fn len(&self) -> u8 {
        self.inner.len()
    }

    /// Returns `true` if this `TinyString` has a length of zero, and `false` otherwise.
    pub const fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Returns the maximum capacity of this vector. This is `N` clamped up to 255.
    pub const fn capacity(&self) -> u8 {
        self.inner.capacity()
    }

    /// Returns a string slice over the contents of this `TinyString`.
    pub fn as_str(&self) -> &str {
        self
    }

    /// Returns a mutable string slice over the contents of this `TinyString`.
    pub fn as_mut_str(&mut self) -> &mut str {
        self
    }

    /// Appends a given string slice onto the end of this `TinyString`, returning how many bytes
    /// were appended.
    ///
    /// This function will attempt to copy as many characters as possible, but will respect UTF-8
    /// char boundaries. This means that if your `TinyString` has only one remaining byte of
    /// capacity and you try to push an 'á' character (whose size is 2 bytes), nothing will occur.
    pub fn push_str(&mut self, string: &str) -> u8 {
        let remaining_capacity = self.capacity() - self.len();

        let byte_count = match string.len() <= remaining_capacity as usize {
            true => string.len(),
            false => string.floor_char_boundary(remaining_capacity as usize),
        };

        unsafe { self.inner.extend_from_slice_copied(string.as_bytes().get_unchecked(0..byte_count)) };
        byte_count as u8
    }

    /// Appends the given [`char`] to the end of this `TinyString`, returning how many bytes were
    /// appended.
    ///
    /// This function will respect UTF-8 char boundaries, so if your `TinyString` has only one
    /// remaining byte of capacity and you try to push an 'á' character (whose size is 2 bytes),
    /// nothing will occur.
    pub fn push(&mut self, ch: char) -> u8 {
        let utf8_len = ch.len_utf8() as u8;
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

    /// Removes the last character from this `TinyString` and returns [`Some`] with it, or
    /// [`None`] if the string was empty.
    pub fn pop(&mut self) -> Option<char> {
        let (new_len, ch) = self.char_indices().next_back()?;
        unsafe { self.inner.set_len(new_len as u8) };
        Some(ch)
    }

    /// Clears this `TinyString`, removing all contents.
    pub fn clear(&mut self) {
        self.inner.clear();
    }

    /// Shortens this `TinyString` to the specified length.
    ///
    /// # Panics
    ///
    /// Panics if `new_len` does not lie on a [`char`] boundary.
    pub fn truncate(&mut self, new_len: u8) {
        if new_len < self.len() {
            if !self.is_char_boundary(new_len as usize) {
                panic!("new_len does not lie on a char boundary");
            }

            self.inner.truncate(new_len);
        }
    }

    /// Returns a mutable reference to this [`TinyString`]'s internal [`TinyVec`].
    ///
    /// # Safety
    ///
    /// This function is unsafe because the returned `&mut TinyVec` allows writing bytes which are
    /// not valid UTF-8. If this constraint is violated, using the original `TinyString` after
    /// dropping the `&mut TinyVec` may violate memory safety, as strings are assumed to be valid
    /// UTF-8.
    pub unsafe fn as_mut_vec(&mut self) -> &mut TinyVec<N, u8> {
        &mut self.inner
    }
}

impl<const N: usize> std::fmt::Write for TinyString<N> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.push_str(s);
        Ok(())
    }

    fn write_char(&mut self, c: char) -> fmt::Result {
        self.push(c);
        Ok(())
    }
}

impl<const N: usize> From<&str> for TinyString<N> {
    fn from(value: &str) -> Self {
        let mut s = TinyString::new();
        s.push_str(value);
        s
    }
}

#[cfg(test)]
mod tests {
    use std::{fmt::Write, ops::Deref};

    const SPANISH: &str = "la brújula léntamente me guía hacia la ubicación de los pingüinos... ¿Cómo y por qué lo hace?";

    use super::TinyString;

    #[test]
    fn test_push_pop() {
        let mut s = TinyString::<255>::new();

        assert_eq!(s.push('a'), 1);
        assert_eq!(s.len(), 1);
        assert_eq!(s.deref(), "a");

        assert_eq!(s.pop(), Some('a'));
        assert_eq!(s.len(), 0);
        assert_eq!(s.deref(), "");

        assert_eq!(s.pop(), None);
        assert_eq!(s.len(), 0);
        assert_eq!(s.deref(), "");

        assert_eq!(s.push_str("höwdý"), 7);
        assert_eq!(s.len(), 7);
        assert_eq!(s.deref(), "höwdý");

        assert_eq!(s.pop(), Some('ý'));
        assert_eq!(s.len(), 5);
        assert_eq!(s.deref(), "höwd");

        assert_eq!(s.push('é'), 2);
        assert_eq!(s.len(), 7);
        assert_eq!(s.deref(), "höwdé");

        assert_eq!(s.pop(), Some('é'));
        assert_eq!(s.len(), 5);
        assert_eq!(s.deref(), "höwd");

        assert_eq!(s.pop(), Some('d'));
        assert_eq!(s.len(), 4);
        assert_eq!(s.deref(), "höw");

        assert_eq!(s.push_str(SPANISH), 101);
        assert_eq!(s.len(), 105);
        assert_eq!(
            s.deref(),
            "höwla brújula léntamente me guía hacia la ubicación de los pingüinos... ¿Cómo y por qué lo hace?"
        );

        assert_eq!(s.push_str(SPANISH), 101);
        assert_eq!(s.len(), 206);
        assert_eq!(s.deref(), "höwla brújula léntamente me guía hacia la ubicación de los pingüinos... ¿Cómo y por qué lo hace?la brújula léntamente me guía hacia la ubicación de los pingüinos... ¿Cómo y por qué lo hace?");

        assert_eq!(s.push_str(SPANISH), 49);
        assert_eq!(s.len(), 255);
        assert_eq!(s.deref(), "höwla brújula léntamente me guía hacia la ubicación de los pingüinos... ¿Cómo y por qué lo hace?la brújula léntamente me guía hacia la ubicación de los pingüinos... ¿Cómo y por qué lo hace?la brújula léntamente me guía hacia la ubicaci");

        assert_eq!(s.push_str(SPANISH), 0);
        assert_eq!(s.len(), 255);
        assert_eq!(s.deref(), "höwla brújula léntamente me guía hacia la ubicación de los pingüinos... ¿Cómo y por qué lo hace?la brújula léntamente me guía hacia la ubicación de los pingüinos... ¿Cómo y por qué lo hace?la brújula léntamente me guía hacia la ubicaci");

        assert_eq!(s.pop(), Some('i'));
        assert_eq!(s.len(), 254);
        assert_eq!(s.deref(), "höwla brújula léntamente me guía hacia la ubicación de los pingüinos... ¿Cómo y por qué lo hace?la brújula léntamente me guía hacia la ubicación de los pingüinos... ¿Cómo y por qué lo hace?la brújula léntamente me guía hacia la ubicac");

        assert_eq!(s.push_str("ácido un placer jajaja"), 0);
        assert_eq!(s.len(), 254);
        assert_eq!(s.deref(), "höwla brújula léntamente me guía hacia la ubicación de los pingüinos... ¿Cómo y por qué lo hace?la brújula léntamente me guía hacia la ubicación de los pingüinos... ¿Cómo y por qué lo hace?la brújula léntamente me guía hacia la ubicac");

        assert_eq!(s.pop(), Some('c'));
        assert_eq!(s.len(), 253);
        assert_eq!(s.deref(), "höwla brújula léntamente me guía hacia la ubicación de los pingüinos... ¿Cómo y por qué lo hace?la brújula léntamente me guía hacia la ubicación de los pingüinos... ¿Cómo y por qué lo hace?la brújula léntamente me guía hacia la ubica");

        assert_eq!(s.push_str("êstúpido"), 2);
        assert_eq!(s.len(), 255);
        assert_eq!(s.deref(), "höwla brújula léntamente me guía hacia la ubicación de los pingüinos... ¿Cómo y por qué lo hace?la brújula léntamente me guía hacia la ubicación de los pingüinos... ¿Cómo y por qué lo hace?la brújula léntamente me guía hacia la ubicaê");
    }

    #[test]
    fn test_clear() {
        let mut s = TinyString::<16>::from("crocante");

        s.clear();
        assert_eq!(s.len(), 0);
        assert_eq!(s.deref(), "");
    }

    #[test]
    fn test_truncate() {
        let mut s = TinyString::<16>::new();

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
        TinyString::<4>::from("ü").truncate(1);
    }

    #[test]
    fn test_write() {
        let mut s = TinyString::<16>::new();

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
        assert_eq!(s.deref(), "Goodbye: 123");
    }
}
