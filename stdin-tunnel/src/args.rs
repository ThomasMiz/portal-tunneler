use core::fmt;
use std::{
    net::IpAddr,
    num::{IntErrorKind, NonZeroU16},
};

pub const DEFAULT_LANE_COUNT: u16 = 5;

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
    pub port_start: NonZeroU16,
    pub remote_address: IpAddr,
    pub remote_port_start: NonZeroU16,
    pub lane_count: NonZeroU16,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ArgumentsError {
    UnknownArgument(String),
    LaneCountError(LaneCountErrorType),
    RemoteAddressError(AddressErrorType),
    InvalidPort(PortErrorType),
    MissingPortStart,
    MissingRemoteAddress,
    MissingRemotePortStart,
}

impl fmt::Display for ArgumentsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownArgument(arg) => write!(f, "Unknown argument: {arg}"),
            Self::LaneCountError(lane_count_error) => lane_count_error.fmt(f),
            Self::RemoteAddressError(remote_address_error) => remote_address_error.fmt(f),
            Self::InvalidPort(port_error) => port_error.fmt(f),
            Self::MissingPortStart => write!(f, "You must specify the first port number to use with -p/--port-start"),
            Self::MissingRemoteAddress => write!(f, "You must specify the remote's public address with -r/--remote-address"),
            Self::MissingRemotePortStart => write!(f, "You must specify the remote's first por number with -q/--remote-port-start"),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum AddressErrorType {
    UnexpectedEnd(String),
    InvalidAddress(String, String),
}

impl fmt::Display for AddressErrorType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedEnd(arg) => write!(f, "Expected address after {arg}"),
            Self::InvalidAddress(arg, addr) => write!(f, "Invalid address after {arg}: {addr}"),
        }
    }
}

fn parse_address_arg(arg: String, maybe_arg2: Option<String>) -> Result<IpAddr, AddressErrorType> {
    let arg2 = match maybe_arg2 {
        Some(value) => value,
        None => return Err(AddressErrorType::UnexpectedEnd(arg)),
    };

    match arg2.parse::<IpAddr>() {
        Ok(addr) => Ok(addr),
        Err(_) => Err(AddressErrorType::InvalidAddress(arg, arg2)),
    }
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
        ArgumentsError::LaneCountError(value)
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

fn parse_port_arg(arg: String, maybe_arg2: Option<String>) -> Result<NonZeroU16, PortErrorType> {
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

pub fn parse_arguments<T>(mut args: T) -> Result<ArgumentsRequest, ArgumentsError>
where
    T: Iterator<Item = String>,
{
    let mut is_server = false;
    let mut lane_count = NonZeroU16::new(DEFAULT_LANE_COUNT).unwrap();
    let mut port_start = None;
    let mut remote_address = None;
    let mut remote_port_start = None;

    // Ignore the first argument, as it's by convention the name of the program
    args.next();

    while let Some(arg) = args.next() {
        if arg.is_empty() {
            continue;
        } else if arg.eq("-h") || arg.eq_ignore_ascii_case("--help") {
            return Ok(ArgumentsRequest::Help);
        } else if arg.eq("-V") || arg.eq_ignore_ascii_case("--version") {
            return Ok(ArgumentsRequest::Version);
        } else if arg.eq("-s") || arg.eq_ignore_ascii_case("--server") {
            is_server = true;
        } else if arg.eq("-c") || arg.eq_ignore_ascii_case("--lane-count") {
            lane_count = parse_lane_count_arg(arg, args.next())?;
        } else if arg.eq("-p") || arg.eq_ignore_ascii_case("--port-start") {
            let port = parse_port_arg(arg, args.next()).map_err(ArgumentsError::InvalidPort)?;
            port_start = Some(port);
        } else if arg.eq("-r") || arg.eq_ignore_ascii_case("--remote-address") {
            let address = parse_address_arg(arg, args.next()).map_err(|e| ArgumentsError::RemoteAddressError(e))?;
            remote_address = Some(address);
        } else if arg.eq("-q") || arg.eq_ignore_ascii_case("--remote-port-start") {
            let port = parse_port_arg(arg, args.next()).map_err(ArgumentsError::InvalidPort)?;
            remote_port_start = Some(port);
        } else {
            return Err(ArgumentsError::UnknownArgument(arg));
        }
    }

    let port_start = port_start.ok_or(ArgumentsError::MissingPortStart)?;
    let remote_address = remote_address.ok_or(ArgumentsError::MissingRemoteAddress)?;
    let remote_port_start = remote_port_start.ok_or(ArgumentsError::MissingRemotePortStart)?;

    let startup_args = StartupArguments {
        is_server,
        port_start,
        remote_address,
        remote_port_start,
        lane_count,
    };

    Ok(ArgumentsRequest::Run(startup_args))
}
