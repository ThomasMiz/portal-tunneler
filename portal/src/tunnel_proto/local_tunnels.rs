use std::{io::Error, net::SocketAddr};

use portal_tunneler_proto::{
    serialize::{ByteRead, ByteWrite},
    shared::address_or_domainname::{AddressOrDomainname, AddressOrDomainnameRef},
};
use tokio::io::{AsyncRead, AsyncWrite};

use super::responses::OpenConnectionError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenLocalConnectionRequest {
    pub target: AddressOrDomainname,
}

impl OpenLocalConnectionRequest {
    pub const fn new(target: AddressOrDomainname) -> Self {
        Self { target }
    }

    pub fn as_ref(&self) -> OpenLocalConnectionRequestRef {
        OpenLocalConnectionRequestRef::new(self.target.as_ref())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenLocalConnectionRequestRef<'a> {
    pub target: AddressOrDomainnameRef<'a>,
}

impl<'a> OpenLocalConnectionRequestRef<'a> {
    pub const fn new(target: AddressOrDomainnameRef<'a>) -> Self {
        Self { target }
    }
}

impl<'a> ByteWrite for OpenLocalConnectionRequestRef<'a> {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), Error> {
        self.target.write(writer).await
    }
}

impl ByteWrite for OpenLocalConnectionRequest {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), Error> {
        self.as_ref().write(writer).await
    }
}

impl ByteRead for OpenLocalConnectionRequest {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, Error> {
        let target = AddressOrDomainname::read(reader).await?;
        Ok(Self { target })
    }
}

#[derive(Debug)]
pub struct OpenLocalConnectionResponse {
    pub result: Result<SocketAddr, (OpenConnectionError, Error)>,
}

impl OpenLocalConnectionResponse {
    pub const fn new(result: Result<SocketAddr, (OpenConnectionError, Error)>) -> Self {
        Self { result }
    }

    pub fn as_ref(&self) -> OpenLocalConnectionResponseRef {
        let result = self
            .result
            .as_ref()
            .map(|sockaddr| *sockaddr)
            .map_err(|(connection_error, error)| (*connection_error, error));

        OpenLocalConnectionResponseRef::new(result)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct OpenLocalConnectionResponseRef<'a> {
    pub result: Result<SocketAddr, (OpenConnectionError, &'a Error)>,
}

impl<'a> OpenLocalConnectionResponseRef<'a> {
    pub const fn new(result: Result<SocketAddr, (OpenConnectionError, &'a Error)>) -> Self {
        Self { result }
    }
}

impl<'a> ByteWrite for OpenLocalConnectionResponseRef<'a> {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), Error> {
        self.result.write(writer).await
    }
}

impl ByteWrite for OpenLocalConnectionResponse {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), Error> {
        self.as_ref().write(writer).await
    }
}

impl ByteRead for OpenLocalConnectionResponse {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, Error> {
        let result = <Result<SocketAddr, (OpenConnectionError, Error)> as ByteRead>::read(reader).await?;
        Ok(Self::new(result))
    }
}
