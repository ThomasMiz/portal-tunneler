use std::{
    io::{self, Error, ErrorKind},
    net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6},
    num::NonZeroU16,
};

use inlined::TinyVec;
use portal_tunneler_proto::{
    serialize::{ByteRead, ByteWrite, U8ReprEnum},
    shared::{AddressOrDomainname, OpenConnectionError},
};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::utils::{read_domainname, UNSPECIFIED_SOCKADDR_V4};

use super::SocksRequestError;

pub const VERSION_BYTE: u8 = 5;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocksStatus {
    Succeeded = 0,
    GeneralFailure = 1,
    NotAllowedByRuleset = 2,
    NetworkUnreachable = 3,
    HostUnreachable = 4,
    ConnectionRefused = 5,
    TTLExpired = 6,
    CommandNotSupported = 7,
    AtypNotSupported = 8,
}

impl From<&Error> for SocksStatus {
    fn from(value: &Error) -> Self {
        match value.kind() {
            ErrorKind::ConnectionAborted | ErrorKind::ConnectionRefused | ErrorKind::ConnectionReset => SocksStatus::ConnectionRefused,
            ErrorKind::NotConnected => SocksStatus::NetworkUnreachable,
            ErrorKind::PermissionDenied => SocksStatus::NotAllowedByRuleset,
            ErrorKind::TimedOut => SocksStatus::HostUnreachable,
            ErrorKind::AddrNotAvailable | ErrorKind::Unsupported => SocksStatus::AtypNotSupported,
            _ => SocksStatus::GeneralFailure,
        }
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocksAtyp {
    IPv4 = 1,
    Domainname = 3,
    IPv6 = 4,
}

impl U8ReprEnum for SocksAtyp {
    fn from_u8(value: u8) -> Option<Self> {
        match value {
            1 => Some(Self::IPv4),
            3 => Some(Self::Domainname),
            4 => Some(Self::IPv6),
            _ => None,
        }
    }

    fn into_u8(self) -> u8 {
        self as u8
    }
}

pub async fn read_request<R, W>(reader: &mut R, writer: &mut W) -> Result<AddressOrDomainname, SocksRequestError>
where
    R: AsyncRead + Unpin + ?Sized,
    W: AsyncWrite + Unpin + ?Sized,
{
    // Read authentication methods the client offers, check for the presence of "no authentication"
    let nmethods = reader.read_u8().await?;
    let mut noauth_found = false;
    for _ in 0..nmethods {
        if reader.read_u8().await? == 0 {
            noauth_found = true;
        }
    }

    if !noauth_found {
        return Err(SocksRequestError::Socks5NoAuthMethodAcceptable);
    }

    // Response to the auth negotiation
    writer.write_all(&[VERSION_BYTE, 0u8]).await?;

    // Connect request: VER
    if reader.read_u8().await? != 5 {
        return Err(SocksRequestError::Socks5InvalidVersion);
    }

    // Connect request: CMD
    if reader.read_u8().await? != 1 {
        return Err(SocksRequestError::Socks5InvalidCommand);
    }

    // Connect request: RSV
    reader.read_u8().await?;

    // Connect request: ATYP
    let atyp_u8 = reader.read_u8().await?;
    let atyp = match SocksAtyp::from_u8(atyp_u8) {
        Some(atyp) => atyp,
        None => return Err(SocksRequestError::Socks5InvalidAtyp(atyp_u8)),
    };

    let target = match atyp {
        SocksAtyp::IPv4 => {
            let mut octets = [0u8; 4];
            reader.read_exact(&mut octets).await?;
            let port = reader.read_u16().await?;

            let ip = Ipv4Addr::from(octets);
            AddressOrDomainname::Address(SocketAddr::V4(SocketAddrV4::new(ip, port)))
        }
        SocksAtyp::IPv6 => {
            let mut octets = [0u8; 16];
            reader.read_exact(&mut octets).await?;
            let port = reader.read_u16().await?;

            let ip = Ipv6Addr::from(octets);
            AddressOrDomainname::Address(SocketAddr::V6(SocketAddrV6::new(ip, port, 0, 0)))
        }
        SocksAtyp::Domainname => {
            let domainname = read_domainname(reader).await?;
            let port = NonZeroU16::read(reader).await?;

            AddressOrDomainname::Domainname(domainname, port)
        }
    };

    Ok(target)
}

impl U8ReprEnum for SocksStatus {
    fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Succeeded),
            1 => Some(Self::GeneralFailure),
            2 => Some(Self::NotAllowedByRuleset),
            3 => Some(Self::NetworkUnreachable),
            4 => Some(Self::HostUnreachable),
            5 => Some(Self::ConnectionRefused),
            6 => Some(Self::TTLExpired),
            7 => Some(Self::CommandNotSupported),
            8 => Some(Self::AtypNotSupported),
            _ => None,
        }
    }

    fn into_u8(self) -> u8 {
        self as u8
    }
}

impl ByteWrite for SocksStatus {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> io::Result<()> {
        self.into_u8().write(writer).await
    }
}

impl ByteRead for SocksStatus {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> io::Result<Self> {
        match Self::from_u8(u8::read(reader).await?) {
            Some(role) => Ok(role),
            None => Err(Error::new(ErrorKind::InvalidData, "Invalid SocksReplyStatus type byte")),
        }
    }
}

pub async fn send_request_error<W>(writer: &mut W, error: &SocksRequestError) -> Result<(), Error>
where
    W: AsyncWrite + Unpin + ?Sized,
{
    match error {
        SocksRequestError::Socks5NoAuthMethodAcceptable => writer.write_all(&[VERSION_BYTE, 0xFFu8]).await,
        SocksRequestError::Socks5InvalidCommand => {
            let rep = SocksStatus::CommandNotSupported as u8;
            let buf = &[VERSION_BYTE, rep, 0, SocksAtyp::IPv4 as u8, 0, 0, 0, 0, 0, 0];
            writer.write_all(buf).await
        }
        SocksRequestError::Socks5InvalidAtyp(_) => {
            let rep = SocksStatus::AtypNotSupported as u8;
            let buf = &[VERSION_BYTE, rep, 0, SocksAtyp::IPv4 as u8, 0, 0, 0, 0, 0, 0];
            writer.write_all(buf).await
        }
        _ => Ok(()),
    }
}

pub async fn send_response<W>(writer: &mut W, result: &Result<SocketAddr, (OpenConnectionError, Error)>) -> Result<(), Error>
where
    W: AsyncWrite + Unpin + ?Sized,
{
    let (rep, bind_address) = match result {
        Ok(address) => (SocksStatus::Succeeded, *address),
        Err((conn_error, error)) => {
            let rep = match conn_error {
                OpenConnectionError::Connect => error.into(),
                _ => SocksStatus::GeneralFailure,
            };

            (rep, UNSPECIFIED_SOCKADDR_V4)
        }
    };

    let mut reply_vec = TinyVec::<22, u8>::new();
    reply_vec.push(VERSION_BYTE);
    reply_vec.push(rep as u8);
    reply_vec.push(0);

    match bind_address {
        SocketAddr::V4(addr4) => {
            reply_vec.push(SocksAtyp::IPv4 as u8);
            reply_vec.extend_from_slice_copied(&addr4.ip().octets());
        }
        SocketAddr::V6(addr6) => {
            reply_vec.push(SocksAtyp::IPv6 as u8);
            reply_vec.extend_from_slice_copied(&addr6.ip().octets());
        }
    }

    reply_vec.extend_from_slice_copied(&bind_address.port().to_be_bytes());

    writer.write_all(&reply_vec).await
}
