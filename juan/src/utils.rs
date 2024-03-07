use std::{
    future::poll_fn,
    io::{self, ErrorKind},
    net::SocketAddr,
    ops::RangeBounds,
    task::Poll,
    time::Instant,
};

use tokio::net::UdpSocket;

/// The same as the `println!` macro, but takes as first parameter a condition on whether to print.
/// If true, the contents will be printed. Otherwise, nothing will happen.
#[macro_export]
macro_rules! printlnif {
    ($condition:expr) => {
        if $condition {
            std::println!();
        }
    };
    ($condition:expr, $($arg:tt)*) => {{
        if $condition {
            std::println!($($arg)*);
        }
    }};
}

/// Sleeps until the provided instant if `Some`, or never finishes if `None`.
pub async fn sleep_until_if_some(until: Option<Instant>) {
    match until {
        Some(v) => tokio::time::sleep_until(tokio::time::Instant::from_std(v)).await,
        None => std::future::pending().await,
    }
}

/// Receives from any [`UdpSocket`], returning the index and the receive result of the first
/// socket to receive something.
///
/// This function never returns an [`ErrorKind::WouldBlock`] error.
pub async fn recv_from_any(sockets: &[UdpSocket], buf: &mut [u8]) -> (usize, io::Result<(usize, SocketAddr)>) {
    loop {
        let (index, readable_result) = poll_fn(|cx| {
            for (index, socket) in sockets.iter().enumerate() {
                if let Poll::Ready(result) = socket.poll_recv_ready(cx) {
                    return Poll::Ready((index, result));
                }
            }

            Poll::Pending
        })
        .await;

        if let Err(error) = readable_result {
            return (index, Err(error));
        }

        match sockets[index].try_recv_from(buf) {
            Ok((len, from)) => return (index, Ok((len, from))),
            Err(error) if error.kind() == ErrorKind::WouldBlock => continue,
            Err(error) => return (index, Err(error)),
        }
    }
}

/// Gets the current system time as a unix timestamp.
///
// Panics with a funny message if the system's date is before 1970.
pub fn get_current_timestamp() -> u64 {
    let now = std::time::SystemTime::now();
    let unix_epoch = std::time::SystemTime::UNIX_EPOCH;
    let duration = now.duration_since(unix_epoch).expect("It is **NOT** 1970, fix your fucking clock");
    duration.as_secs()
}

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
