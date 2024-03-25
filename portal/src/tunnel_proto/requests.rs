use std::io::{Error, ErrorKind};

use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};

use super::{
    serialize::{ByteRead, ByteWrite},
    u8_repr_enum::U8ReprEnum,
};

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
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), Error> {
        writer.write_u8(self.into_u8()).await
    }
}

impl ByteRead for ClientStreamRequest {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, Error> {
        match Self::from_u8(u8::read(reader).await?) {
            Some(role) => Ok(role),
            None => Err(Error::new(ErrorKind::InvalidData, "Invalid ClientStreamRequest type byte")),
        }
    }
}
