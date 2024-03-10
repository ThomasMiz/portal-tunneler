use std::{
    fmt,
    num::{IntErrorKind, NonZeroU16},
};

use super::ArgumentsError;

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

pub(super) fn parse_lane_count_arg(arg: String, maybe_arg2: Option<String>) -> Result<NonZeroU16, LaneCountErrorType> {
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

pub(super) fn parse_port_number_arg(arg: String, maybe_arg2: Option<String>) -> Result<NonZeroU16, PortErrorType> {
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
