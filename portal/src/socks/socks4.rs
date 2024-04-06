use std::{
    io::{Error, ErrorKind},
    net::{Ipv4Addr, SocketAddr, SocketAddrV4},
    num::NonZeroU16,
};

use portal_tunneler_proto::{
    serialize::{ByteRead, U8ReprEnum},
    shared::{AddressOrDomainname, OpenConnectionError},
};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::utils::read_nullterm_domainname;

use super::SocksRequestError;

pub const VERSION_BYTE: u8 = 4;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResultCode {
    Granted = 90,
    RejectedOrFailed = 91,
}

impl U8ReprEnum for ResultCode {
    fn from_u8(value: u8) -> Option<Self> {
        match value {
            90 => Some(Self::Granted),
            91 => Some(Self::RejectedOrFailed),
            _ => None,
        }
    }

    fn into_u8(self) -> u8 {
        self as u8
    }
}

pub async fn read_request<R>(reader: &mut R) -> Result<AddressOrDomainname, SocksRequestError>
where
    R: AsyncRead + Unpin + ?Sized,
{
    // Command code
    let cd = reader.read_u8().await?;
    if cd != 1 {
        return Err(SocksRequestError::Socks4InvalidCommand(cd));
    }

    // Destination port
    let dst_port = NonZeroU16::read(reader).await?;

    // Destination IP
    let mut dst_ip_octets = [0u8; 4];
    reader.read_exact(&mut dst_ip_octets).await?;

    // User ID (ignore all bytes up to the null termination)
    while reader.read_u8().await? != 0 {}

    // If the IP address is three zeroes, as per SOCKS4A, the client is specifying a domainname
    let target = if dst_ip_octets[..3] == [0, 0, 0] {
        if dst_ip_octets[3] == 0 {
            return Err(Error::new(ErrorKind::Other, "Client specified IPv4 address 0.0.0.0").into());
        }

        AddressOrDomainname::Domainname(read_nullterm_domainname(reader).await?, dst_port)
    } else {
        AddressOrDomainname::Address(SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::from(dst_ip_octets), dst_port.get())))
    };

    Ok(target)
}

pub async fn send_request_error<W>(writer: &mut W, error: &SocksRequestError) -> Result<(), Error>
where
    W: AsyncWrite + Unpin + ?Sized,
{
    if let SocksRequestError::Socks4InvalidCommand(_) = error {
        let buf = [0, ResultCode::RejectedOrFailed as u8, 0, 0, 0, 0, 0, 0];
        writer.write_all(&buf).await
    } else {
        Ok(())
    }
}

pub async fn send_response<W>(writer: &mut W, result: &Result<SocketAddr, (OpenConnectionError, Error)>) -> Result<(), Error>
where
    W: AsyncWrite + Unpin + ?Sized,
{
    let result_code = match result {
        Ok(_) => ResultCode::Granted,
        Err(_) => ResultCode::RejectedOrFailed,
    };

    let buf = [0, result_code as u8, 0, 0, 0, 0, 0, 0];
    writer.write_all(&buf).await
}
