use std::{
    future::poll_fn,
    io::{self, ErrorKind},
    net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6},
    task::Poll,
};

use tokio::net::UdpSocket;

/// An empty IPv4 [`SocketAddr`] with port 0
pub const UNSPECIFIED_SOCKADDR_V4: SocketAddr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0));

/// An empty IPv6 [`SocketAddr`] with port, flowinfo, and scope_id all set to 0.
pub const UNSPECIFIED_SOCKADDR_V6: SocketAddr = SocketAddr::V6(SocketAddrV6::new(Ipv6Addr::UNSPECIFIED, 0, 0, 0));

/// Returns whether a string is a valid domain name, checking the string's length and characters.
///
/// This is not intended to be a fully correct implementation, but rather used to rule out strings
/// that clearly do not follow the correct format. This method has false positives, but no false
/// negatives.
pub fn is_valid_domainname(s: &str) -> bool {
    (1..256).contains(&s.len()) && s.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'.' || c == b'-')
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
