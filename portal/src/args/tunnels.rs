use std::{
    fmt,
    net::{IpAddr, SocketAddr},
    num::NonZeroU16,
};

use portal_tunneler_proto::shared::{AddressOrDomainname, TunnelSide, TunnelSpec, TunnelTarget};

use crate::utils;

#[derive(Debug, PartialEq, Eq)]
pub enum TunnelSpecErrorType {
    UnexpectedEnd(String),
    InvalidFormat(String, String),
    InvalidPort(String, String),
    InvalidAddress(String, String),
}

impl fmt::Display for TunnelSpecErrorType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedEnd(arg) => write!(f, "Expected tunnel specification after {arg}"),
            Self::InvalidFormat(arg, arg2) => write!(f, "Invalid tunnel specification after {arg}: {arg2}"),
            Self::InvalidPort(arg, arg2) => write!(f, "Invalid port after {arg}: {arg2}"),
            Self::InvalidAddress(arg, arg2) => write!(f, "Invalid IP address or domain name after {arg}: {arg2}"),
        }
    }
}

/// Parses a port number at the end of the string, walking down up to a colon ':' or the start of
/// the string.
///
/// Returns an error if the port is invalid, otherwise returns ownership of `arg` and `spec`, as
/// well as specifying whether a colon index was found, by returning a tuple with
/// `(arg, spec, maybe_colon_index, port)`.
fn parse_port_backwards(
    arg: String,
    spec: String,
    end_index: usize,
) -> Result<(String, String, Option<usize>, NonZeroU16), TunnelSpecErrorType> {
    let s = &spec[..end_index];
    let maybe_colon_index = s.bytes().enumerate().rev().find(|(_, c)| *c == b':').map(|(i, _)| i);
    let start_index = maybe_colon_index.map(|i| i + 1).unwrap_or(0);

    match s[start_index..].parse::<NonZeroU16>() {
        Ok(port) => Ok((arg, spec, maybe_colon_index, port)),
        Err(_) => Err(TunnelSpecErrorType::InvalidPort(
            arg,
            utils::cut_string(spec, start_index..end_index),
        )),
    }
}

/// Parses an address at the end of the string, walking down up to a colon ':' or the start of the
/// string.
///
/// Returns an error if the address is invalid, otherwise returns ownership of `arg` and `spec`,
/// as well as specifying whether a colon index was found, by returning a tuple with
/// `(arg, spec, maybe_colon_index, address)`.
fn parse_address_backwards(
    arg: String,
    spec: String,
    end_index: usize,
    port: NonZeroU16,
) -> Result<(String, String, Option<usize>, AddressOrDomainname), TunnelSpecErrorType> {
    let s = &spec[..end_index];

    let mut needs_close_bracket = false;
    let mut saw_bracket = false;
    let mut index = s.len();
    let mut maybe_colon_index = None;
    while index != 0 && maybe_colon_index.is_none() {
        index -= 1;
        match s.as_bytes()[index] {
            b':' if !needs_close_bracket => maybe_colon_index = Some(index),
            b']' => {
                needs_close_bracket = true;
                saw_bracket = true;
            }
            b'[' => needs_close_bracket = false,
            _ => {}
        }
    }

    let start_index = maybe_colon_index.map(|i| i + 1).unwrap_or(0);

    let s = if saw_bracket {
        if s.len() < 3 {
            return Err(TunnelSpecErrorType::InvalidFormat(arg, spec));
        }

        &s[(start_index + 1)..(s.len() - 1)]
    } else {
        &s[start_index..]
    };

    let address = match s.parse::<IpAddr>() {
        Ok(addr) => AddressOrDomainname::Address(SocketAddr::new(addr, port.get())),
        Err(_) if utils::is_valid_domainname(s) => AddressOrDomainname::Domainname(String::from(s), port),
        Err(_) => {
            return Err(TunnelSpecErrorType::InvalidAddress(
                arg,
                utils::cut_string(spec, start_index..end_index),
            ))
        }
    };

    Ok((arg, spec, maybe_colon_index, address))
}

/// Parses a tunnel specification argument in an SSH-like format. The specification may be fully
/// within the first argument (e.g. "-L8080:localhost:8080") or as a separate argument (e.g.
/// "-L 8080:localhost:8080"). The second argument is only consumed if necessary.
pub(super) fn parse_tunnel_spec_arg<F>(
    side: TunnelSide,
    mut arg: String,
    start_index: usize,
    index: usize,
    get_next_arg: F,
) -> Result<TunnelSpec, TunnelSpecErrorType>
where
    F: FnOnce() -> Option<String>,
{
    let spec = if start_index == arg.len() {
        match get_next_arg() {
            Some(s) => s,
            None => return Err(TunnelSpecErrorType::UnexpectedEnd(arg)),
        }
    } else {
        let s = String::from(&arg[start_index..]);
        arg.truncate(start_index);
        s
    };

    let end_index = spec.len();
    let (arg, spec, maybe_colon_index, last_port) = parse_port_backwards(arg, spec, end_index)?;
    let last_colon_index = match maybe_colon_index {
        Some(i) => i,
        None => {
            return Ok(TunnelSpec {
                index,
                side,
                target: TunnelTarget::Socks,
                listen_address: AddressOrDomainname::Domainname(String::from("localhost"), last_port),
            })
        }
    };

    let (arg, spec, maybe_colon_index, address) = parse_address_backwards(arg, spec, last_colon_index, last_port)?;
    let last_colon_index = match maybe_colon_index {
        Some(i) => i,
        None => {
            return Ok(TunnelSpec {
                index,
                side,
                target: TunnelTarget::Socks,
                listen_address: address,
            })
        }
    };

    let target_address = address;
    let (arg, spec, maybe_colon_index, first_port) = parse_port_backwards(arg, spec, last_colon_index)?;
    let last_colon_index = match maybe_colon_index {
        Some(i) => i,
        None => {
            return Ok(TunnelSpec {
                index,
                side,
                target: TunnelTarget::Address(target_address),
                listen_address: AddressOrDomainname::Domainname(String::from("localhost"), first_port),
            })
        }
    };

    let (arg, spec, maybe_colon_index, address) = parse_address_backwards(arg, spec, last_colon_index, first_port)?;
    if maybe_colon_index.is_some() {
        return Err(TunnelSpecErrorType::InvalidFormat(arg, spec));
    }

    Ok(TunnelSpec {
        index,
        side,
        target: TunnelTarget::Address(target_address),
        listen_address: address,
    })
}
