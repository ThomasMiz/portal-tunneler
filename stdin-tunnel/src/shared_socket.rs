use std::{io, ops::Deref, sync::Arc};

#[derive(Debug, Clone)]
pub struct SharedUdpSocket {
    arc: Arc<Inner>,
}

#[derive(Debug)]
struct Inner {
    io: tokio::net::UdpSocket,
}

impl Deref for SharedUdpSocket {
    type Target = tokio::net::UdpSocket;

    fn deref(&self) -> &Self::Target {
        &self.arc.io
    }
}

impl SharedUdpSocket {
    pub fn new(socket: tokio::net::UdpSocket) -> io::Result<Self> {
        let arc = Arc::new(Inner { io: socket });
        Ok(Self { arc })
    }
}
