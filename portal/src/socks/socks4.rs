use std::{io::Error, net::SocketAddr};

use portal_tunneler_proto::shared::{AddressOrDomainname, OpenConnectionError};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use super::SocksRequestError;

pub const VERSION_BYTE: u8 = 4;

pub async fn read_request<R, W>(reader: &mut R, writer: &mut W) -> Result<AddressOrDomainname, SocksRequestError>
where
    R: AsyncRead + Unpin + ?Sized,
    W: AsyncWrite + Unpin + ?Sized,
{
    unimplemented!()
}

pub async fn send_request_error<W>(writer: &mut W, error: &SocksRequestError) -> Result<(), Error>
where
    W: AsyncWrite + Unpin + ?Sized,
{
    unimplemented!()
}

pub async fn send_response<W>(writer: &mut W, result: &Result<SocketAddr, (OpenConnectionError, Error)>) -> Result<(), Error>
where
    W: AsyncWrite + Unpin + ?Sized,
{
    unimplemented!()
}
