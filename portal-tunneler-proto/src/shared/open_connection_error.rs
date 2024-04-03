use std::{
    fmt,
    io::{self, Error, ErrorKind},
};

use crate::serialize::{ByteRead, ByteWrite, U8ReprEnum};
use tokio::io::{AsyncRead, AsyncWrite};

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenConnectionError {
    BindSocket = 0,
    DNSQuery = 1,
    Connect = 2,
}

impl fmt::Display for OpenConnectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BindSocket => write!(f, "bind socket"),
            Self::DNSQuery => write!(f, "DNS query"),
            Self::Connect => write!(f, "connect"),
        }
    }
}

impl U8ReprEnum for OpenConnectionError {
    fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::BindSocket),
            1 => Some(Self::DNSQuery),
            2 => Some(Self::Connect),
            _ => None,
        }
    }

    fn into_u8(self) -> u8 {
        self as u8
    }
}

impl ByteWrite for OpenConnectionError {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> io::Result<()> {
        self.into_u8().write(writer).await
    }
}

impl ByteRead for OpenConnectionError {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> io::Result<Self> {
        match Self::from_u8(u8::read(reader).await?) {
            Some(role) => Ok(role),
            None => Err(Error::new(ErrorKind::InvalidData, "Invalid OpenConnectionError type byte")),
        }
    }
}
