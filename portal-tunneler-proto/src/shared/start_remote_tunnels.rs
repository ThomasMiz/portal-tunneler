use std::io::Error;

use crate::{
    serialize::{ByteRead, ByteWrite},
    shared::{AddressOrDomainname, AddressOrDomainnameRef, RemoteTunnelID, TunnelTargetType},
};

use tokio::io::{AsyncRead, AsyncWrite};

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
