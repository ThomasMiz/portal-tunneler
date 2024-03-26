use std::{
    hash::Hash,
    mem::MaybeUninit,
    ops::{Deref, DerefMut},
};

/// A UTF-8–encoded, inline string of up to [`u8::MAX`] characters. Similar to
/// [`InlineString`](super::InlineString) in that it stores chars inline instead of allocating on
/// the heap, but has a set capacity of 255 and the length is an `u8` instead of an `usize`.
pub struct TinyString {
    len: u8,
    inner: [MaybeUninit<u8>; u8::MAX as usize],
}

impl Deref for TinyString {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        unsafe { std::mem::transmute(self.inner.get_unchecked(..(self.len as usize))) }
    }
}

impl DerefMut for TinyString {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { std::mem::transmute(self.inner.get_unchecked_mut(..(self.len as usize))) }
    }
}

impl std::fmt::Debug for TinyString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(self.deref(), f)
    }
}

impl std::fmt::Display for TinyString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(self.deref(), f)
    }
}

impl Clone for TinyString {
    #[allow(clippy::non_canonical_clone_impl)]
    fn clone(&self) -> Self {
        let mut result = Self {
            len: self.len,
            inner: unsafe { MaybeUninit::uninit().assume_init() },
        };

        unsafe { std::ptr::copy_nonoverlapping(self.inner.as_ptr(), result.inner.as_mut_ptr(), self.len as usize) };

        result
    }
}

impl Copy for TinyString {}

impl Default for TinyString {
    fn default() -> Self {
        Self::new()
    }
}

impl PartialEq for TinyString {
    fn eq(&self, other: &Self) -> bool {
        self.deref().eq(other.deref())
    }
}

impl Eq for TinyString {}

impl PartialOrd for TinyString {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TinyString {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.deref().cmp(other.deref())
    }
}

impl Hash for TinyString {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.deref().hash(state)
    }
}

impl TinyString {
    /// Creates a new empty `TinyString`.
    pub const fn new() -> Self {
        Self {
            len: 0,
            inner: unsafe { MaybeUninit::uninit().assume_init() },
        }
    }

    /// Returns the length of this `TinyString`, in bytes.
    pub const fn len(&self) -> u8 {
        self.len
    }

    /// Appends a given string slice onto the end of this `TinyString`, returning how many bytes
    /// were appended.
    ///
    /// This function will attempt to copy as many characters as possible, but will respect UTF-8
    /// char boundaries. This means that if your `TinyString` has only one remaining byte of
    /// capacity and you try to push an 'á' character (whose size is 2 bytes), nothing will occur.
    pub fn push_str(&mut self, string: &str) -> u8 {
        let remaining_capacity = u8::MAX - self.len();

        let byte_count = match remaining_capacity as usize >= string.len() {
            true => string.len(),
            false => string.floor_char_boundary(remaining_capacity as usize),
        };

        if byte_count != 0 {
            unsafe {
                std::ptr::copy_nonoverlapping(
                    string.as_ptr(),
                    self.inner.get_unchecked_mut(self.len as usize).as_mut_ptr(),
                    byte_count,
                );
            }

            self.len += byte_count as u8;
        }

        byte_count as u8
    }

    /// Appends the given [`char`] to the end of this `TinyString`, returning how many bytes were
    /// appended.
    ///
    /// This function will respect UTF-8 char boundaries, so if your `TinyString` has only one
    /// remaining byte of capacity and you try to push an 'á' character (whose size is 2 bytes),
    /// nothing will occur.
    pub fn push(&mut self, ch: char) -> u8 {
        let mut buf = [0; 4];
        let ch_bytes = ch.encode_utf8(&mut buf).as_bytes();
        let ch_len = ch_bytes.len() as u8;

        if ch_len > u8::MAX - self.len {
            return 0;
        }

        unsafe {
            std::ptr::copy_nonoverlapping(
                ch_bytes.as_ptr(),
                self.inner.get_unchecked_mut(self.len as usize).as_mut_ptr(),
                ch_bytes.len(),
            );
        }

        self.len += ch_len;
        ch_len
    }

    /// Removes the last character from this `TinyString` and returns [`Some`] with it, or [`None`]
    /// if the string was empty.
    pub fn pop(&mut self) -> Option<char> {
        let (new_len, ch) = self.char_indices().next_back()?;
        self.len = new_len as u8;
        Some(ch)
    }

    /// Clears this `TinyString`, removing all contents.
    pub fn clear(&mut self) {
        self.len = 0;
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

            self.len = new_len;
        }
    }

    /// Gets a mutable reference to this `TinyString`'s raw underlying buffer. This operation is
    /// unsafe, and the caller is responsible for ensuring this type's invariants are maintaned.
    ///
    /// # Safety
    ///
    /// - The final contents of the buffer must be valid UTF-8.
    pub unsafe fn as_mut_buffer(&mut self) -> &mut [MaybeUninit<u8>; u8::MAX as usize] {
        &mut self.inner
    }

    /// Forces the length of this `TinyString` to `new_len`. This operation is unsafe, and the
    /// caller is responsible for ensuring this type's invariants are maintaned.
    ///
    /// # Safety
    ///
    /// - The elements in between the old and new lengths must be initialized.
    /// - The final contents of the buffer must be valid UTF-8.
    pub unsafe fn set_len(&mut self, new_len: u8) {
        self.len = new_len;
    }
}

impl std::fmt::Write for TinyString {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        self.push_str(s);
        Ok(())
    }

    fn write_char(&mut self, c: char) -> std::fmt::Result {
        self.push(c);
        Ok(())
    }
}

impl From<&str> for TinyString {
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
        let mut s = TinyString::new();

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
        let mut s = TinyString::from("crocante");

        s.clear();
        assert_eq!(s.len(), 0);
        assert_eq!(s.deref(), "");
    }

    #[test]
    fn test_truncate() {
        let mut s = TinyString::new();

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
        TinyString::from("ü").truncate(1);
    }

    #[test]
    fn test_write() {
        let mut s = TinyString::new();

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
