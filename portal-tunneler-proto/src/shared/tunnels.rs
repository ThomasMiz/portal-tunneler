use std::{
    fmt,
    io::{self, Error, ErrorKind},
};

use tokio::io::{AsyncRead, AsyncWrite};

use crate::serialize::{ByteRead, ByteWrite, U8ReprEnum};

use super::address_or_domainname::AddressOrDomainname;

/// Specifies an SSH-like TCP tunnel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TunnelSpec {
    /// The order in which the tunnel specifications were specified by parameters. This starts at
    /// zero and increments sequentially. This is used for printing error messages.
    pub index: usize,

    /// The side which will listen for incoming TCP connections.
    pub side: TunnelSide,

    /// The target to which the TCP connections will be forwarded to on the other side.
    pub target: TunnelTarget,

    /// The address or addresses to listen for incoming TCP connection at.
    pub listen_address: AddressOrDomainname,
}

/// Represents the possible sides for a tunnel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TunnelSide {
    /// We locally listen for incoming connections and forward them to the remote.
    Local,

    /// The remote listens for incoming connections and forwards them to us.
    Remote,
}

/// Represents the possible targets to which a TCP tunnel can forward a TCP connection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TunnelTarget {
    /// Forward to an address or domain name with port.
    Address(AddressOrDomainname),

    /// Forward to wherever the connection specifies using the SOCKS proxy protocol.
    Socks,
}

impl TunnelTarget {
    /// Gets this [`TunnelTarget`]'s respective [`TunnelTargetType`] value.
    pub fn as_type(&self) -> TunnelTargetType {
        match self {
            Self::Address(_) => TunnelTargetType::Static,
            Self::Socks => TunnelTargetType::Socks,
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
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> io::Result<()> {
        self.into_u8().write(writer).await
    }
}

impl ByteRead for TunnelTargetType {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> io::Result<Self> {
        match Self::from_u8(u8::read(reader).await?) {
            Some(role) => Ok(role),
            None => Err(Error::new(ErrorKind::InvalidData, "Invalid TunnelTargetType type byte")),
        }
    }
}

/// An number that uniquely identifies a remote tunnel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RemoteTunnelID(pub u32);

impl ByteRead for RemoteTunnelID {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> io::Result<Self> {
        Ok(Self(u32::read(reader).await?))
    }
}

impl ByteWrite for RemoteTunnelID {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> io::Result<()> {
        self.0.write(writer).await
    }
}
