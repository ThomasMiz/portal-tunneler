use std::{
    io::{Error, ErrorKind},
    net::{SocketAddr, SocketAddrV4, SocketAddrV6},
};

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use super::{
    serialize::{ByteRead, ByteWrite, SmallReadString, SmallWriteString},
    u8_repr_enum::U8ReprEnum,
};

pub enum AddressOrDomainname {
    Address(SocketAddr),
    Domainname(String, u16),
}

impl ByteWrite for AddressOrDomainname {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), Error> {
        match self {
            Self::Address(addr) => addr.write(writer).await,
            Self::Domainname(domainname, port) => (200u8, SmallWriteString(domainname), *port).write(writer).await,
        }
    }
}

impl ByteRead for AddressOrDomainname {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, Error> {
        let addr_type = reader.read_u8().await?;
        match addr_type {
            4 => Ok(AddressOrDomainname::Address(SocketAddr::V4(SocketAddrV4::read(reader).await?))),
            6 => Ok(AddressOrDomainname::Address(SocketAddr::V6(SocketAddrV6::read(reader).await?))),
            200 => Ok(AddressOrDomainname::Domainname(
                SmallReadString::read(reader).await?.0,
                u16::read(reader).await?,
            )),
            v => Err(Error::new(ErrorKind::InvalidData, format!("Invalid AddressOrDomainName type, {v}"))),
        }
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartConnectionError {
    BindSocketFailed = 0,
    ConnectFailed = 1,
}

impl U8ReprEnum for StartConnectionError {
    fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::BindSocketFailed),
            1 => Some(Self::ConnectFailed),
            _ => None,
        }
    }

    fn into_u8(self) -> u8 {
        self as u8
    }
}

impl ByteWrite for StartConnectionError {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), Error> {
        writer.write_u8(self.into_u8()).await
    }
}

impl ByteRead for StartConnectionError {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, Error> {
        match StartConnectionError::from_u8(u8::read(reader).await?) {
            Some(role) => Ok(role),
            None => Err(Error::new(ErrorKind::InvalidData, "Invalid StartConnectionError type byte")),
        }
    }
}
