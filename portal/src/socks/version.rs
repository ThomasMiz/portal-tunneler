use std::io::{self, Error, ErrorKind};

use portal_tunneler_proto::serialize::{ByteRead, ByteWrite, U8ReprEnum};
use tokio::io::{AsyncRead, AsyncWrite};

use super::{socks4, socks5};

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocksVersion {
    Four = socks4::VERSION_BYTE,
    Five = socks5::VERSION_BYTE,
}

impl U8ReprEnum for SocksVersion {
    fn from_u8(value: u8) -> Option<Self> {
        match value {
            socks4::VERSION_BYTE => Some(Self::Four),
            socks5::VERSION_BYTE => Some(Self::Five),
            _ => None,
        }
    }

    fn into_u8(self) -> u8 {
        self as u8
    }
}

impl ByteWrite for SocksVersion {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> io::Result<()> {
        self.into_u8().write(writer).await
    }
}

impl ByteRead for SocksVersion {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> io::Result<Self> {
        match Self::from_u8(u8::read(reader).await?) {
            Some(role) => Ok(role),
            None => Err(Error::new(ErrorKind::InvalidData, "Invalid SocksVersion type byte")),
        }
    }
}
