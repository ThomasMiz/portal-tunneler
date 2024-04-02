use std::{
    fmt::Write,
    future::poll_fn,
    io::{self, Error, ErrorKind},
    net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6},
    task::Poll,
};

use inlined::{CompactVec, InlineString};
use portal_tunneler_proto::shared::address_or_domainname::AddressOrDomainnameRef;
use tokio::net::{TcpListener, TcpStream, UdpSocket};

/// An empty IPv4 [`SocketAddr`] with port 0
pub const UNSPECIFIED_SOCKADDR_V4: SocketAddr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0));

/// An empty IPv6 [`SocketAddr`] with port, flowinfo, and scope_id all set to 0.
pub const UNSPECIFIED_SOCKADDR_V6: SocketAddr = SocketAddr::V6(SocketAddrV6::new(Ipv6Addr::UNSPECIFIED, 0, 0, 0));

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

pub async fn accept_from_any(listeners: &[TcpListener]) -> (usize, io::Result<(TcpStream, SocketAddr)>) {
    loop {
        let (index, result) = poll_fn(|cx| {
            for (index, listener) in listeners.iter().enumerate() {
                if let Poll::Ready(result) = listener.poll_accept(cx) {
                    return Poll::Ready((index, result));
                }
            }

            Poll::Pending
        })
        .await;

        if !result.as_ref().is_err_and(|error| error.kind() == ErrorKind::WouldBlock) {
            return (index, result);
        }
    }
}

pub async fn bind_listeners<'a>(address: AddressOrDomainnameRef<'a>) -> io::Result<CompactVec<3, TcpListener>> {
    match address {
        AddressOrDomainnameRef::Address(address) => Ok(CompactVec::from(TcpListener::bind(address).await?)),
        AddressOrDomainnameRef::Domainname(domainname, port) => {
            let mut s = InlineString::<262>::new();
            let _ = write!(s, "{domainname}:{port}");

            let addresses = tokio::net::lookup_host(s.as_str()).await?;

            let mut listeners = CompactVec::new();
            let mut last_error = None;

            for address in addresses {
                let bind_result = TcpListener::bind(address).await;
                match bind_result {
                    Ok(listener) => listeners.push(listener),
                    Err(error) => last_error = Some(error),
                }
            }

            if listeners.is_empty() {
                Err(last_error.unwrap_or_else(|| {
                    let msg = format!("The domainname \"{s}\" could not be resolved to any addresses");
                    Error::new(ErrorKind::InvalidInput, msg)
                }))
            } else {
                Ok(listeners)
            }
        }
    }
}

pub async fn bind_connect<'a>(address: AddressOrDomainnameRef<'a>) -> io::Result<TcpStream> {
    match address {
        AddressOrDomainnameRef::Address(address) => TcpStream::connect(address).await,
        AddressOrDomainnameRef::Domainname(domainname, port) => {
            let mut s = InlineString::<262>::new();
            let _ = write!(s, "{domainname}:{port}");

            TcpStream::connect(s.as_str()).await
        }
    }
}
