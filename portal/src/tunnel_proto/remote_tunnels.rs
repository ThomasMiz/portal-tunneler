use std::{io::Error, net::SocketAddr};

use portal_tunneler_proto::{
    serialize::{ByteRead, ByteWrite},
    shared::{
        address_or_domainname::{AddressOrDomainname, AddressOrDomainnameRef},
        tunnels::{RemoteTunnelID, TunnelTargetType},
    },
};
use tokio::io::{AsyncRead, AsyncWrite};

use super::responses::OpenConnectionError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartRemoteTunnelRequest {
    pub tunnel_id: RemoteTunnelID,
    pub target_type: TunnelTargetType,
    pub listen_at: AddressOrDomainname,
}

impl StartRemoteTunnelRequest {
    pub const fn new(tunnel_id: RemoteTunnelID, target_type: TunnelTargetType, listen_at: AddressOrDomainname) -> Self {
        Self {
            tunnel_id,
            target_type,
            listen_at,
        }
    }

    pub fn as_ref(&self) -> StartRemoteTunnelRequestRef {
        StartRemoteTunnelRequestRef::new(self.tunnel_id, self.target_type, self.listen_at.as_ref())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StartRemoteTunnelRequestRef<'a> {
    pub tunnel_id: RemoteTunnelID,
    pub target_type: TunnelTargetType,
    pub listen_at: AddressOrDomainnameRef<'a>,
}

impl<'a> StartRemoteTunnelRequestRef<'a> {
    pub const fn new(tunnel_id: RemoteTunnelID, target_type: TunnelTargetType, listen_at: AddressOrDomainnameRef<'a>) -> Self {
        Self {
            tunnel_id,
            target_type,
            listen_at,
        }
    }
}

impl<'a> ByteWrite for StartRemoteTunnelRequestRef<'a> {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), Error> {
        (self.tunnel_id, self.target_type, &self.listen_at).write(writer).await
    }
}

impl ByteWrite for StartRemoteTunnelRequest {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), Error> {
        self.as_ref().write(writer).await
    }
}

impl ByteRead for StartRemoteTunnelRequest {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, Error> {
        let tunnel_id = RemoteTunnelID::read(reader).await?;
        let target_type = TunnelTargetType::read(reader).await?;
        let listen_at = AddressOrDomainname::read(reader).await?;

        Ok(StartRemoteTunnelRequest {
            tunnel_id,
            target_type,
            listen_at,
        })
    }
}

#[derive(Debug)]
pub struct StartRemoteTunnelResponse {
    pub result: Result<(), Error>,
}

impl StartRemoteTunnelResponse {
    pub const fn new(result: Result<(), Error>) -> Self {
        Self { result }
    }

    pub fn as_ref(&self) -> StartRemoteTunnelResponseRef {
        StartRemoteTunnelResponseRef::new(self.result.as_ref().map(|_| ()))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct StartRemoteTunnelResponseRef<'a> {
    pub result: Result<(), &'a Error>,
}

impl<'a> StartRemoteTunnelResponseRef<'a> {
    pub const fn new(result: Result<(), &'a Error>) -> Self {
        Self { result }
    }
}

impl<'a> ByteWrite for StartRemoteTunnelResponseRef<'a> {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), Error> {
        self.result.write(writer).await
    }
}

impl ByteWrite for StartRemoteTunnelResponse {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> Result<(), Error> {
        self.as_ref().write(writer).await
    }
}

impl ByteRead for StartRemoteTunnelResponse {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> Result<Self, Error> {
        let result = <Result<(), Error> as ByteRead>::read(reader).await?;
        Ok(Self { result })
    }
}

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
