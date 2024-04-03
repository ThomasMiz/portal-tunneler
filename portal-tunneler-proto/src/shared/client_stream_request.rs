use std::io::{self, Error, ErrorKind};

use crate::serialize::{ByteRead, ByteWrite, U8ReprEnum};
use tokio::io::{AsyncRead, AsyncWrite};

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientStreamRequest {
    OpenLocalTunnelConnection = 0,
    StartRemoteTunnels = 1,
}

impl U8ReprEnum for ClientStreamRequest {
    fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::OpenLocalTunnelConnection),
            1 => Some(Self::StartRemoteTunnels),
            _ => None,
        }
    }

    fn into_u8(self) -> u8 {
        self as u8
    }
}

impl ByteWrite for ClientStreamRequest {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> io::Result<()> {
        self.into_u8().write(writer).await
    }
}

impl ByteRead for ClientStreamRequest {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> io::Result<Self> {
        match Self::from_u8(u8::read(reader).await?) {
            Some(role) => Ok(role),
            None => Err(Error::new(ErrorKind::InvalidData, "Invalid ClientStreamRequest type byte")),
        }
    }
}
