use core::fmt;
use std::{
    net::{IpAddr, SocketAddr},
    num::{IntErrorKind, NonZeroU16},
};

use crate::utils::{cut_string, is_valid_domainname};

pub const DEFAULT_LANE_COUNT: NonZeroU16 = unsafe { NonZeroU16::new_unchecked(5) };

pub fn get_version_string() -> String {
    format!(
        concat!(env!("CARGO_PKG_NAME"), " ", env!("CARGO_PKG_VERSION"), " ({} {})"),
        std::env::consts::OS,
        std::env::consts::ARCH
    )
}

pub fn get_help_string() -> &'static str {
    "I need somebody"
}

#[derive(Debug, PartialEq)]
pub enum ArgumentsRequest {
    Help,
    Version,
    Run(StartupArguments),
}

#[derive(Debug, PartialEq)]
pub struct StartupArguments {
    pub is_server: bool,
    pub my_ip: Option<IpAddr>,
    pub port_start: Option<NonZeroU16>,
    pub lane_count: NonZeroU16,
    pub tunnels: Vec<TunnelSpec>,
}

/// Specifies an SSH-like TCP tunnel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TunnelSpec {
    /// The side which will listen for incoming TCP connections.
    pub side: TunnelSide,

    /// The target to which the TCP connections will be forwarded to on the other side.
    pub target: TunnelTarget,

    /// The address or addresses to listen for incoming TCP connection at.
    pub address: AddressOrDomainname,
}

/// Either [`SocketAddr`] or a domainname, which is a string composed of at most 256 followed by a
/// ":port"
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AddressOrDomainname {
    Address(SocketAddr),
    Domainname(String),
}

/// Represents the possible sides for a tunnel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TunnelSide {
    /// We locally listen for incoming connections and forward them to the remote.
    Local,

    /// The remote listens for incoming connections and forwards them to us.
    Remote,
}

/// Represents the possible targets to which a TCP tunnel can forward a TCP connection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TunnelTarget {
    /// Forward to an address or domain name with port.
    Address(AddressOrDomainname),

    /// Forward to wherever the connection specifies using the SOCKS proxy protocol.
    Socks,
}

impl StartupArguments {
    pub fn empty() -> Self {
        Self {
            is_server: false,
            my_ip: None,
            port_start: None,
            lane_count: DEFAULT_LANE_COUNT,
            tunnels: Vec::new(),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum ArgumentsError {
    UnknownArgument(String),
    MyIpError(IpAddrErrorType),
    LaneCount(LaneCountErrorType),
    PortStart(PortErrorType),
    LocalTunnel(TunnelSpecErrorType),
    RemoteTunnel(TunnelSpecErrorType),
}

impl fmt::Display for ArgumentsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownArgument(arg) => write!(f, "Unknown argument: {arg}"),
            Self::MyIpError(ip_error) => ip_error.fmt(f),
            Self::LaneCount(lane_count_error) => lane_count_error.fmt(f),
            Self::PortStart(port_start_error) => port_start_error.fmt(f),
            Self::LocalTunnel(tunnel_spec_error) => tunnel_spec_error.fmt(f),
            Self::RemoteTunnel(tunnel_spec_error) => tunnel_spec_error.fmt(f),
        }
    }
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

fn parse_ip_addr_arg(arg: String, maybe_arg2: Option<String>) -> Result<IpAddr, IpAddrErrorType> {
    let arg2 = match maybe_arg2 {
        Some(arg2) => arg2,
        None => return Err(IpAddrErrorType::UnexpectedEnd(arg)),
    };

    arg2.parse::<IpAddr>().map_err(|_| IpAddrErrorType::InvalidValue(arg, arg2))
}

#[derive(Debug, PartialEq, Eq)]
pub enum LaneCountErrorType {
    UnexpectedEnd(String),
    MustBeGreaterThanZero(String, String),
    TooLarge(String, String),
    InvalidValue(String, String),
}

impl fmt::Display for LaneCountErrorType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedEnd(arg) => write!(f, "Expected integer after {arg}"),
            Self::MustBeGreaterThanZero(arg, arg2) => write!(f, "Lane count must be greater than 0 after {arg}: {arg2}"),
            Self::TooLarge(arg, arg2) => write!(f, "Lane count must be at most 16 bits after {arg}: {arg2}"),
            Self::InvalidValue(arg, arg2) => write!(f, "Invalid lane count value after {arg}: {arg2}"),
        }
    }
}

impl From<LaneCountErrorType> for ArgumentsError {
    fn from(value: LaneCountErrorType) -> Self {
        ArgumentsError::LaneCount(value)
    }
}

fn parse_lane_count_arg(arg: String, maybe_arg2: Option<String>) -> Result<NonZeroU16, LaneCountErrorType> {
    let arg2 = match maybe_arg2 {
        Some(arg2) => arg2,
        None => return Err(LaneCountErrorType::UnexpectedEnd(arg)),
    };

    arg2.parse::<NonZeroU16>().map_err(|parse_int_error| match parse_int_error.kind() {
        IntErrorKind::Zero | IntErrorKind::NegOverflow => LaneCountErrorType::MustBeGreaterThanZero(arg, arg2),
        IntErrorKind::PosOverflow => LaneCountErrorType::TooLarge(arg, arg2),
        _ => LaneCountErrorType::InvalidValue(arg, arg2),
    })
}

#[derive(Debug, PartialEq, Eq)]
pub enum PortErrorType {
    UnexpectedEnd(String),
    MustBeGreaterThanZero(String, String),
    TooLarge(String, String),
    InvalidValue(String, String),
}

impl fmt::Display for PortErrorType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedEnd(arg) => write!(f, "Expected integer after {arg}"),
            Self::MustBeGreaterThanZero(arg, arg2) => write!(f, "Port number must be greater than 0 after {arg}: {arg2}"),
            Self::TooLarge(arg, arg2) => write!(f, "Port number must be at most 16 bits after {arg}: {arg2}"),
            Self::InvalidValue(arg, arg2) => write!(f, "Invalid port number after {arg}: {arg2}"),
        }
    }
}

fn parse_port_number_arg(arg: String, maybe_arg2: Option<String>) -> Result<NonZeroU16, PortErrorType> {
    let arg2 = match maybe_arg2 {
        Some(arg2) => arg2,
        None => return Err(PortErrorType::UnexpectedEnd(arg)),
    };

    arg2.parse::<NonZeroU16>().map_err(|parse_int_error| match parse_int_error.kind() {
        IntErrorKind::Zero | IntErrorKind::NegOverflow => PortErrorType::MustBeGreaterThanZero(arg, arg2),
        IntErrorKind::PosOverflow => PortErrorType::TooLarge(arg, arg2),
        _ => PortErrorType::InvalidValue(arg, arg2),
    })
}

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

fn parse_tunnel_spec_arg<F>(
    side: TunnelSide,
    mut arg: String,
    start_index: usize,
    get_next_arg: F,
) -> Result<TunnelSpec, TunnelSpecErrorType>
where
    F: FnOnce() -> Option<String>,
{
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
            Err(_) => Err(TunnelSpecErrorType::InvalidPort(arg, cut_string(spec, start_index..end_index))),
        }
    }

    fn parse_address_backwards(
        arg: String,
        spec: String,
        end_index: usize,
        port: u16,
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
            Ok(addr) => AddressOrDomainname::Address(SocketAddr::new(addr, port)),
            Err(_) if is_valid_domainname(s) => AddressOrDomainname::Domainname(format!("{s}:{port}")),
            Err(_) => return Err(TunnelSpecErrorType::InvalidAddress(arg, cut_string(spec, start_index..end_index))),
        };

        Ok((arg, spec, maybe_colon_index, address))
    }

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
                side,
                target: TunnelTarget::Socks,
                address: AddressOrDomainname::Domainname(format!("localhost:{last_port}")),
            })
        }
    };

    let (arg, spec, maybe_colon_index, address) = parse_address_backwards(arg, spec, last_colon_index, last_port.get())?;
    let last_colon_index = match maybe_colon_index {
        Some(i) => i,
        None => {
            return Ok(TunnelSpec {
                side,
                target: TunnelTarget::Socks,
                address,
            })
        }
    };

    let target_address = address;
    let (arg, spec, maybe_colon_index, first_port) = parse_port_backwards(arg, spec, last_colon_index)?;
    let last_colon_index = match maybe_colon_index {
        Some(i) => i,
        None => {
            return Ok(TunnelSpec {
                side,
                target: TunnelTarget::Address(target_address),
                address: AddressOrDomainname::Domainname(format!("localhost:{first_port}")),
            })
        }
    };

    let (arg, spec, maybe_colon_index, address) = parse_address_backwards(arg, spec, last_colon_index, first_port.get())?;
    if maybe_colon_index.is_some() {
        return Err(TunnelSpecErrorType::InvalidFormat(arg, spec));
    }

    Ok(TunnelSpec {
        side,
        target: TunnelTarget::Address(target_address),
        address,
    })
}

pub fn parse_arguments<T>(mut args: T) -> Result<ArgumentsRequest, ArgumentsError>
where
    T: Iterator<Item = String>,
{
    let mut result = StartupArguments::empty();

    // Ignore the first argument, as it's by convention the name of the program
    args.next();

    while let Some(arg) = args.next() {
        if arg.is_empty() {
            continue;
        } else if arg.eq("-h") || arg.eq_ignore_ascii_case("--help") {
            return Ok(ArgumentsRequest::Help);
        } else if arg.eq("-V") || arg.eq_ignore_ascii_case("--version") {
            return Ok(ArgumentsRequest::Version);
        } else if arg.eq("--server") {
            result.is_server = true;
        } else if arg.eq("-a") || arg.eq_ignore_ascii_case("--my-ip") {
            result.my_ip = Some(parse_ip_addr_arg(arg, args.next()).map_err(ArgumentsError::MyIpError)?);
        } else if arg.eq("-c") || arg.eq_ignore_ascii_case("--lane-count") {
            result.lane_count = parse_lane_count_arg(arg, args.next())?;
        } else if arg.eq("-p") || arg.eq_ignore_ascii_case("--port-start") {
            result.port_start = Some(parse_port_number_arg(arg, args.next()).map_err(ArgumentsError::PortStart)?);
        } else if arg.starts_with("-L") {
            let spec_result = parse_tunnel_spec_arg(TunnelSide::Local, arg, 2, || args.next());
            result.tunnels.push(spec_result.map_err(ArgumentsError::LocalTunnel)?);
        } else if arg.eq("--local-tunnel") {
            let spec_result = parse_tunnel_spec_arg(TunnelSide::Local, arg, 14, || args.next());
            result.tunnels.push(spec_result.map_err(ArgumentsError::LocalTunnel)?);
        } else if arg.starts_with("-R") {
            let spec_result = parse_tunnel_spec_arg(TunnelSide::Remote, arg, 2, || args.next());
            result.tunnels.push(spec_result.map_err(ArgumentsError::RemoteTunnel)?);
        } else if arg.eq("--remote-tunnel") {
            let spec_result = parse_tunnel_spec_arg(TunnelSide::Remote, arg, 15, || args.next());
            result.tunnels.push(spec_result.map_err(ArgumentsError::RemoteTunnel)?);
        } else {
            return Err(ArgumentsError::UnknownArgument(arg));
        }
    }

    Ok(ArgumentsRequest::Run(result))
}
