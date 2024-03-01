use std::{future::poll_fn, io, net::SocketAddr, task::Poll, time::Instant};

use tokio::{io::ReadBuf, net::UdpSocket};

/// Sleeps until the provided instant if `Some`, or never finishes if `None`.
pub async fn sleep_until_if_some(until: Option<Instant>) {
    match until {
        Some(v) => tokio::time::sleep_until(tokio::time::Instant::from_std(v)).await,
        None => std::future::pending().await,
    }
}

pub async fn recv_from_any_with_id<'a, F, I, D>(f: F, buf: &mut [u8]) -> (D, io::Result<(usize, SocketAddr)>)
where
    F: Fn() -> I,
    I: Iterator<Item = (D, &'a UdpSocket)>,
{
    let mut read_buf = ReadBuf::new(buf);

    poll_fn(|cx| {
        for (id, socket) in f() {
            match socket.poll_recv_from(cx, &mut read_buf) {
                Poll::Ready(result) => return Poll::Ready((id, result.map(|addr| (read_buf.filled().len(), addr)))),
                Poll::Pending => {}
            }
        }

        Poll::Pending
    })
    .await
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
