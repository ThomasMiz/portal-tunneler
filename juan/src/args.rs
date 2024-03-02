use core::fmt;
use std::{
    io::ErrorKind,
    net::{SocketAddr, ToSocketAddrs},
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
    pub lane_count: NonZeroU16,
}

impl StartupArguments {
    pub fn empty() -> Self {
        Self {
            is_server: false,
            lane_count: NonZeroU16::new(DEFAULT_LANE_COUNT).unwrap(),
        }
    }

    pub fn fill_empty_fields_with_defaults(&mut self) {}
}

#[derive(Debug, PartialEq, Eq)]
pub enum ArgumentsError {
    UnknownArgument(String),
    LaneCountError(LaneCountErrorType),
}

impl fmt::Display for ArgumentsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownArgument(arg) => write!(f, "Unknown argument: {arg}"),
            Self::LaneCountError(lane_count_error) => lane_count_error.fmt(f),
        }
    }
}

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

fn parse_socket_arg(
    result_vec: &mut Vec<SocketAddr>,
    arg: String,
    maybe_arg2: Option<String>,
    default_port: u16,
) -> Result<(), SocketErrorType> {
    let arg2 = match maybe_arg2 {
        Some(value) => value,
        None => return Err(SocketErrorType::UnexpectedEnd(arg)),
    };

    let iter = match arg2.to_socket_addrs() {
        Ok(iter) => iter,
        Err(err) if err.kind() == ErrorKind::InvalidInput => match format!("{arg2}:{default_port}").to_socket_addrs() {
            Ok(iter) => iter,
            Err(_) => return Err(SocketErrorType::InvalidSocketAddress(arg, arg2)),
        },
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
        } else if arg.eq("-c") || arg.eq_ignore_ascii_case("--lane-count") {
            result.lane_count = parse_lane_count_arg(arg, args.next())?;
        } else if arg.eq("--server") {
            result.is_server = true;
        } else {
            return Err(ArgumentsError::UnknownArgument(arg));
        }
    }

    result.fill_empty_fields_with_defaults();
    Ok(ArgumentsRequest::Run(result))
}
