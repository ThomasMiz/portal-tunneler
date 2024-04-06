use std::{
    io::{Error, ErrorKind},
    net::SocketAddr,
};

use portal_tunneler_proto::{
    serialize::U8ReprEnum,
    shared::{AddressOrDomainname, OpenConnectionError},
};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite};

use self::version::SocksVersion;

mod socks4;
mod socks5;
mod version;

#[derive(Debug)]
pub enum SocksRequestError {
    IO(Error),
    InvalidVersion(u8),
    Socks4InvalidCommand(u8),
    Socks5NoAuthMethodAcceptable,
    Socks5InvalidVersion(u8),
    Socks5InvalidCommand(u8),
    Socks5InvalidAtyp(u8),
}

impl std::fmt::Display for SocksRequestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IO(error) => error.fmt(f),
            Self::InvalidVersion(ver) => write!(f, "Client requested invalid SOCKS version: {ver}"),
            Self::Socks4InvalidCommand(cmd) => write!(f, "Client requested invalid SOCKS4 command: {cmd}"),
            Self::Socks5NoAuthMethodAcceptable => write!(f, "No acceptable SOCKS5 authentication method"),
            Self::Socks5InvalidVersion(ver) => write!(f, "Client requested SOCKS5, but then specified another version: {ver}"),
            Self::Socks5InvalidCommand(cmd) => write!(f, "Client requested invalid SOCKS5 command: {cmd}"),
            Self::Socks5InvalidAtyp(atyp) => write!(f, "Client requested invalid SOCKS5 address type: {atyp}"),
        }
    }
}

impl From<SocksRequestError> for Error {
    fn from(value: SocksRequestError) -> Self {
        match value {
            SocksRequestError::IO(error) => error,
            other => Error::new(ErrorKind::Other, format!("{other}")),
        }
    }
}

impl From<Error> for SocksRequestError {
    fn from(value: Error) -> Self {
        Self::IO(value)
    }
}

pub async fn read_request<R, W>(reader: &mut R, writer: &mut W) -> Result<(SocksVersion, AddressOrDomainname), SocksRequestError>
where
    R: AsyncRead + Unpin + ?Sized,
    W: AsyncWrite + Unpin + ?Sized,
{
    let version_u8 = reader.read_u8().await?;
    let version = SocksVersion::from_u8(version_u8).ok_or(SocksRequestError::InvalidVersion(version_u8))?;

    let target = match version {
        SocksVersion::Four => socks4::read_request(reader).await?,
        SocksVersion::Five => socks5::read_request(reader, writer).await?,
    };

    Ok((version, target))
}

pub async fn send_request_error<W>(writer: &mut W, error: &SocksRequestError) -> Result<(), Error>
where
    W: AsyncWrite + Unpin + ?Sized,
{
    match error {
        SocksRequestError::Socks4InvalidCommand(_) => socks4::send_request_error(writer, error).await,
        SocksRequestError::Socks5InvalidAtyp(_)
        | SocksRequestError::Socks5InvalidCommand(_)
        | SocksRequestError::Socks5InvalidVersion(_)
        | SocksRequestError::Socks5NoAuthMethodAcceptable => socks5::send_request_error(writer, error).await,
        _ => Ok(()),
    }
}

pub async fn send_response<W>(
    writer: &mut W,
    version: SocksVersion,
    result: &Result<SocketAddr, (OpenConnectionError, Error)>,
) -> Result<(), Error>
where
    W: AsyncWrite + Unpin + ?Sized,
{
    match version {
        SocksVersion::Four => socks4::send_response(writer, result).await,
        SocksVersion::Five => socks5::send_response(writer, result).await,
    }
}
