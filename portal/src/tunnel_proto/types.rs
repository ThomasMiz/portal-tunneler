use std::{
    fmt::{self, Write},
    io::{self, Error, ErrorKind},
    net::{SocketAddr, SocketAddrV4, SocketAddrV6},
    num::NonZeroU16,
};

use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

use super::{
    serialize::{ByteRead, ByteWrite, SmallReadString, SmallWriteString},
    u8_repr_enum::U8ReprEnum,
};

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
    pub async fn bind_listener(&mut self) -> io::Result<TcpListener> {
        match self {
            Self::Address(address) => TcpListener::bind(*address).await, // TODO: This should bind all the sockets the address yields!
            Self::Domainname(domainname, port) => {
                let original_length = domainname.len();
                let _ = write!(domainname, ":{port}");

                let result = TcpListener::bind(domainname.as_str()).await;
                domainname.truncate(original_length);

                result
            }
        }
    }

    // TODO: This code really shouldn't be here
    pub async fn bind_connect(&mut self) -> io::Result<TcpStream> {
        match self {
            Self::Address(address) => TcpStream::connect(*address).await,
            Self::Domainname(domainname, port) => {
                let original_length = domainname.len();
                let _ = write!(domainname, ":{port}");

                let result = TcpStream::connect(domainname.as_str()).await;
                domainname.truncate(original_length);

                result
            }
        }
    }
}

impl fmt::Display for AddressOrDomainname {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Address(address) => address.fmt(f),
            Self::Domainname(domainname, port) => write!(f, "{domainname}:{port}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddressOrDomainnameRef<'a> {
    Address(&'a SocketAddr),
    Domainname(&'a str, NonZeroU16),
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

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientStreamRequest {
    NewLocalTunnelConnection = 0,
    OpenRemoteTunnels = 1,
}

impl U8ReprEnum for ClientStreamRequest {
    fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::NewLocalTunnelConnection),
            1 => Some(Self::OpenRemoteTunnels),
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

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartConnectionError {
    BindSocket = 0,
    DNSQuery = 1,
    Connect = 2,
}

impl fmt::Display for StartConnectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BindSocket => write!(f, "bind socket"),
            Self::DNSQuery => write!(f, "DNS query"),
            Self::Connect => write!(f, "connect"),
        }
    }
}

impl U8ReprEnum for StartConnectionError {
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

impl ByteWrite for StartConnectionError {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), Error> {
        writer.write_u8(self.into_u8()).await
    }
}

impl ByteRead for StartConnectionError {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, Error> {
        match Self::from_u8(u8::read(reader).await?) {
            Some(role) => Ok(role),
            None => Err(Error::new(ErrorKind::InvalidData, "Invalid StartConnectionError type byte")),
        }
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TunnelTargetType {
    Static = 0,
    Socks = 1,
}

impl fmt::Display for TunnelTargetType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Static => write!(f, "static"),
            Self::Socks => write!(f, "SOCKS"),
        }
    }
}

impl U8ReprEnum for TunnelTargetType {
    fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Static),
            1 => Some(Self::Socks),
            _ => None,
        }
    }

    fn into_u8(self) -> u8 {
        self as u8
    }
}

impl ByteWrite for TunnelTargetType {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), Error> {
        writer.write_u8(self.into_u8()).await
    }
}

impl ByteRead for TunnelTargetType {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, Error> {
        match Self::from_u8(u8::read(reader).await?) {
            Some(role) => Ok(role),
            None => Err(Error::new(ErrorKind::InvalidData, "Invalid TunnelTargetType type byte")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenRemoteTunnelRequest {
    pub tunnel_id: u32,
    pub target_type: TunnelTargetType,
    pub listen_at: AddressOrDomainname,
}

impl OpenRemoteTunnelRequest {
    pub const fn new(tunnel_id: u32, target_type: TunnelTargetType, listen_at: AddressOrDomainname) -> Self {
        Self {
            tunnel_id,
            target_type,
            listen_at,
        }
    }

    pub fn as_ref(&self) -> OpenRemoteTunnelRequestRef {
        OpenRemoteTunnelRequestRef::new(self.tunnel_id, self.target_type, self.listen_at.as_ref())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpenRemoteTunnelRequestRef<'a> {
    pub tunnel_id: u32,
    pub target_type: TunnelTargetType,
    pub listen_at: AddressOrDomainnameRef<'a>,
}

impl<'a> OpenRemoteTunnelRequestRef<'a> {
    pub const fn new(tunnel_id: u32, target_type: TunnelTargetType, listen_at: AddressOrDomainnameRef<'a>) -> Self {
        Self {
            tunnel_id,
            target_type,
            listen_at,
        }
    }
}

impl<'a> ByteWrite for OpenRemoteTunnelRequestRef<'a> {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), Error> {
        (self.tunnel_id, self.target_type, &self.listen_at).write(writer).await
    }
}

impl ByteWrite for OpenRemoteTunnelRequest {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), Error> {
        self.as_ref().write(writer).await
    }
}

impl ByteRead for OpenRemoteTunnelRequest {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, Error> {
        let tunnel_id = reader.read_u32().await?;
        let target_type = TunnelTargetType::read(reader).await?;
        let listen_at = AddressOrDomainname::read(reader).await?;

        Ok(OpenRemoteTunnelRequest {
            tunnel_id,
            target_type,
            listen_at,
        })
    }
}
