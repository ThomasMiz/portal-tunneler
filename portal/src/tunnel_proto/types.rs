use std::{
    fmt::{self, Write},
    io::{self, Error, ErrorKind},
    net::{SocketAddr, SocketAddrV4, SocketAddrV6},
    num::NonZeroU16,
};

use inlined::InlineString;
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite},
    net::{TcpListener, TcpStream},
};

use super::serialize::{ByteRead, ByteWrite, SmallReadString, SmallWriteString};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AddressOrDomainname {
    Address(SocketAddr),
    Domainname(String, NonZeroU16),
}

impl AddressOrDomainname {
    pub fn as_ref(&self) -> AddressOrDomainnameRef {
        match self {
            Self::Address(address) => AddressOrDomainnameRef::Address(address),
            Self::Domainname(domainname, port) => AddressOrDomainnameRef::Domainname(domainname, *port),
        }
    }

    // TODO: This code really shouldn't be here
    pub async fn bind_listener(&self) -> io::Result<TcpListener> {
        match self {
            Self::Address(address) => TcpListener::bind(*address).await, // TODO: This should bind all the sockets the address yields!
            Self::Domainname(domainname, port) => {
                let mut s = InlineString::<262>::new();
                let _ = write!(s, "{domainname}:{port}");

                TcpListener::bind(s.as_str()).await
            }
        }
    }

    // TODO: This code really shouldn't be here
    pub async fn bind_connect(&self) -> io::Result<TcpStream> {
        match self {
            Self::Address(address) => TcpStream::connect(*address).await,
            Self::Domainname(domainname, port) => {
                let mut s = InlineString::<262>::new();
                let _ = write!(s, "{domainname}:{port}");

                TcpStream::connect(s.as_str()).await
            }
        }
    }
}

impl fmt::Display for AddressOrDomainname {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.as_ref(), f)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddressOrDomainnameRef<'a> {
    Address(&'a SocketAddr),
    Domainname(&'a str, NonZeroU16),
}

impl<'a> fmt::Display for AddressOrDomainnameRef<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Address(address) => address.fmt(f),
            Self::Domainname(domainname, port) => write!(f, "{domainname}:{port}"),
        }
    }
}

impl<'a> ByteWrite for AddressOrDomainnameRef<'a> {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), Error> {
        match self {
            Self::Address(addr) => addr.write(writer).await,
            Self::Domainname(domainname, port) => (200u8, SmallWriteString(domainname), *port).write(writer).await,
        }
    }
}

impl ByteWrite for AddressOrDomainname {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), Error> {
        self.as_ref().write(writer).await
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
                NonZeroU16::read(reader).await?,
            )),
            v => Err(Error::new(ErrorKind::InvalidData, format!("Invalid AddressOrDomainName type, {v}"))),
        }
    }
}
