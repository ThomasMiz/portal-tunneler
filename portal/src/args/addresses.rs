use std::{
    fmt::{self, Write},
    io::ErrorKind,
    net::{IpAddr, SocketAddr, ToSocketAddrs},
};

use crate::utils::InlineString;

#[derive(Debug, PartialEq, Eq)]
pub enum SocketErrorType {
    UnexpectedEnd(String),
    InvalidSocketAddress(String, String),
}

impl fmt::Display for SocketErrorType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedEnd(arg) => write!(f, "Expected socket address after {arg}"),
            Self::InvalidSocketAddress(arg, addr) => write!(f, "Invalid socket address after {arg}: {addr}"),
        }
    }
}

/// Parses a socket address argument, doing domain name resolution if necessary. The resulting
/// [`SocketAddr`] instances are pushed onto a `result_vec` vector.
pub(super) fn parse_socket_arg(
    result_vec: &mut Vec<SocketAddr>,
    arg: String,
    maybe_arg2: Option<String>,
    default_port: u16,
) -> Result<(), SocketErrorType> {
    let arg2 = match maybe_arg2 {
        Some(value) => value,
        None => return Err(SocketErrorType::UnexpectedEnd(arg)),
    };

    // TODO: Make this support IPv6 without brackets (e.g. "--arg ::1" instead of "--arg [::1]")
    let iter = match arg2.to_socket_addrs() {
        Ok(iter) => iter,
        Err(err) if err.kind() == ErrorKind::InvalidInput => {
            let mut s = InlineString::<262>::new();
            let _ = write!(s, "{arg2}:{default_port}");
            match s.to_socket_addrs() {
                Ok(iter) => iter,
                Err(_) => return Err(SocketErrorType::InvalidSocketAddress(arg, arg2)),
            }
        }
        Err(_) => return Err(SocketErrorType::InvalidSocketAddress(arg, arg2)),
    };

    for sockaddr in iter {
        if !result_vec.contains(&sockaddr) {
            result_vec.push(sockaddr);
        }
    }

    Ok(())
}

#[derive(Debug, PartialEq, Eq)]
pub enum IpAddrErrorType {
    UnexpectedEnd(String),
    InvalidValue(String, String),
}

impl fmt::Display for IpAddrErrorType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedEnd(arg) => write!(f, "Expected IP address after {arg}"),
            Self::InvalidValue(arg, arg2) => write!(f, "Invalid IP address after {arg}: {arg2}"),
        }
    }
}

/// Parses an IP address argument. Domain names are not accepted by this function, as it is
/// intended to be used in places where only a single IP address is allowed.
pub(super) fn parse_ip_addr_arg(arg: String, maybe_arg2: Option<String>) -> Result<IpAddr, IpAddrErrorType> {
    let arg2 = match maybe_arg2 {
        Some(arg2) => arg2,
        None => return Err(IpAddrErrorType::UnexpectedEnd(arg)),
    };

    arg2.parse::<IpAddr>().map_err(|_| IpAddrErrorType::InvalidValue(arg, arg2))
}
