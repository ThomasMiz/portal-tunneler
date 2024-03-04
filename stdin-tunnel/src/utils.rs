use std::{
    future::poll_fn,
    io::{self, ErrorKind},
    net::SocketAddr,
    task::Poll,
    time::Instant,
};

use tokio::net::UdpSocket;

/// Sleeps until the provided instant if `Some`, or never finishes if `None`.
pub async fn sleep_until_if_some(until: Option<Instant>) {
    match until {
        Some(v) => tokio::time::sleep_until(tokio::time::Instant::from_std(v)).await,
        None => std::future::pending().await,
    }
}

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
