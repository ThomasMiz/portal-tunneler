use std::{
    fmt,
    io::{Error, ErrorKind},
};

use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};

use super::{
    serialize::{ByteRead, ByteWrite},
    u8_repr_enum::U8ReprEnum,
};

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
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), Error> {
        writer.write_u8(self.into_u8()).await
    }
}

impl ByteRead for OpenConnectionError {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, Error> {
        match Self::from_u8(u8::read(reader).await?) {
            Some(role) => Ok(role),
            None => Err(Error::new(ErrorKind::InvalidData, "Invalid StartConnectionError type byte")),
        }
    }
}
