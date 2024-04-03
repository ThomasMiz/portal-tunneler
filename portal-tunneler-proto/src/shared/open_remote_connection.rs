use std::{io::Error, net::SocketAddr};

use crate::{
    serialize::{ByteRead, ByteWrite},
    shared::{AddressOrDomainname, AddressOrDomainnameRef, OpenConnectionError, RemoteTunnelID},
};
use tokio::io::{AsyncRead, AsyncWrite};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenRemoteConnectionRequest {
    pub tunnel_id: RemoteTunnelID,
    pub maybe_target: Option<AddressOrDomainname>,
}

impl OpenRemoteConnectionRequest {
    pub const fn new(tunnel_id: RemoteTunnelID, maybe_target: Option<AddressOrDomainname>) -> Self {
        Self { tunnel_id, maybe_target }
    }

    pub fn as_ref(&self) -> OpenRemoteConnectionRequestRef {
        OpenRemoteConnectionRequestRef::new(self.tunnel_id, self.maybe_target.as_ref().map(|mt| mt.as_ref()))
    }
}

pub struct OpenRemoteConnectionRequestRef<'a> {
    pub tunnel_id: RemoteTunnelID,
    pub maybe_target: Option<AddressOrDomainnameRef<'a>>,
}

impl<'a> OpenRemoteConnectionRequestRef<'a> {
    pub const fn new(tunnel_id: RemoteTunnelID, maybe_target: Option<AddressOrDomainnameRef<'a>>) -> Self {
        Self { tunnel_id, maybe_target }
    }
}

impl<'a> ByteWrite for OpenRemoteConnectionRequestRef<'a> {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), Error> {
        (self.tunnel_id, self.maybe_target).write(writer).await
    }
}

impl ByteWrite for OpenRemoteConnectionRequest {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), Error> {
        self.as_ref().write(writer).await
    }
}

impl ByteRead for OpenRemoteConnectionRequest {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, Error> {
        let tunnel_id = RemoteTunnelID::read(reader).await?;
        let maybe_target = <Option<AddressOrDomainname> as ByteRead>::read(reader).await?;

        Ok(Self { tunnel_id, maybe_target })
    }
}

#[derive(Debug)]
pub struct OpenRemoteConnectionResponse {
    pub result: Result<SocketAddr, (OpenConnectionError, Error)>,
}

impl OpenRemoteConnectionResponse {
    pub const fn new(result: Result<SocketAddr, (OpenConnectionError, Error)>) -> Self {
        Self { result }
    }

    pub fn as_ref(&self) -> OpenRemoteConnectionResponseRef {
        let result = self
            .result
            .as_ref()
            .map(|sockaddr| *sockaddr)
            .map_err(|(connection_error, error)| (*connection_error, error));

        OpenRemoteConnectionResponseRef::new(result)
    }
}

#[derive(Debug)]
pub struct OpenRemoteConnectionResponseRef<'a> {
    pub result: Result<SocketAddr, (OpenConnectionError, &'a Error)>,
}

impl<'a> OpenRemoteConnectionResponseRef<'a> {
    pub const fn new(result: Result<SocketAddr, (OpenConnectionError, &'a Error)>) -> Self {
        Self { result }
    }
}

impl<'a> ByteWrite for OpenRemoteConnectionResponseRef<'a> {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), Error> {
        self.result.write(writer).await
    }
}

impl ByteWrite for OpenRemoteConnectionResponse {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), Error> {
        self.as_ref().write(writer).await
    }
}

impl ByteRead for OpenRemoteConnectionResponse {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, Error> {
        let result = <Result<SocketAddr, (OpenConnectionError, Error)> as ByteRead>::read(reader).await?;
        Ok(Self { result })
    }
}
