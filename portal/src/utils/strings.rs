use std::{
    io::{Error, ErrorKind},
    ops::RangeBounds,
};

use tokio::io::{AsyncRead, AsyncReadExt};

/// Returns whether a string is a valid domain name, checking the string's length and characters.
///
/// This is not intended to be a fully correct implementation, but rather used to rule out strings
/// that clearly do not follow the correct format. This method has false positives, but no false
/// negatives.
pub fn is_valid_domainname(s: &str) -> bool {
    (1..256).contains(&s.len()) && s.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'.' || c == b'-')
}

/// Returns the same string, with all the characters outside the range stripped out.
pub fn cut_string<R: RangeBounds<usize>>(mut s: String, range: R) -> String {
    let start_index = match range.start_bound() {
        std::ops::Bound::Included(i) => *i,
        std::ops::Bound::Excluded(i) => *i + 1,
        std::ops::Bound::Unbounded => 0,
    };

    let end_index = match range.end_bound() {
        std::ops::Bound::Included(i) => *i + 1,
        std::ops::Bound::Excluded(i) => *i,
        std::ops::Bound::Unbounded => s.len(),
    };

    if !s.is_char_boundary(start_index) || !s.is_char_boundary(end_index) {
        panic!("The specified range does not split the string across UTF-8 char boundaries");
    }

    unsafe {
        // SAFETY: We previously ensured the range splits the bytes across char boundaries,
        // thus ensuring the slice in the range is, on its own, valid UTF-8.
        let vec = s.as_mut_vec();
        vec.copy_within(range, 0);
        vec.truncate(end_index - start_index);
    }

    s
}

/// Asynchronously reads a domainname string from an [`AsyncRead`].
pub async fn read_domainname<R>(reader: &mut R) -> Result<String, Error>
where
    R: AsyncRead + Unpin + ?Sized,
{
    let length = reader.read_u8().await? as usize;
    if length == 0 {
        return Err(Error::new(ErrorKind::InvalidData, "Domainname length cannot be 0"));
    }

    let mut s = String::with_capacity(length as usize);

    unsafe {
        let buf = s.as_mut_vec();
        buf.set_len(length);
        // SAFETY: We ensure the bytes read into the string are valid UTF-8 by checking that they
        // are graphical ASCII values, which are all valid UTF-8.

        let mut count = 0;
        while count < length {
            let more = reader.read(&mut buf[count..length]).await?;

            for c in &buf[count..(count + more)] {
                if !c.is_ascii_alphanumeric() && *c != b'-' && *c != b'.' {
                    return Err(Error::new(
                        ErrorKind::InvalidData,
                        format!("Domainname contains invalid character: {c}"),
                    ));
                }
            }

            count += more;
        }
    }

    Ok(s)
}
