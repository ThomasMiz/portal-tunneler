//! An implementation of [`AsyncUdpSocket`] from an [`Arc`]-ed [`tokio::net::UdpSocket`], so said
//! socket can be used by a [`quinn::Endpoint`] at the same time as other processes.
//!
//! Based on the code in quinn::runtime::tokio

use std::{
    io,
    ops::Deref,
    sync::Arc,
    task::{ready, Context, Poll},
};

use quinn::AsyncUdpSocket;
use tokio::io::Interest;

#[derive(Debug, Clone)]
pub struct SharedUdpSocket {
    arc: Arc<Inner>,
}

#[derive(Debug)]
struct Inner {
    io: tokio::net::UdpSocket,
    state: quinn_udp::UdpSocketState,
}

impl Deref for SharedUdpSocket {
    type Target = tokio::net::UdpSocket;

    fn deref(&self) -> &Self::Target {
        &self.arc.io
    }
}

impl SharedUdpSocket {
    pub fn new(socket: tokio::net::UdpSocket) -> io::Result<Self> {
        quinn_udp::UdpSocketState::configure((&socket).into())?;

        let arc = Arc::new(Inner {
            io: socket,
            state: quinn_udp::UdpSocketState::new(),
        });
        Ok(Self { arc })
    }
}

impl AsyncUdpSocket for SharedUdpSocket {
    fn poll_send(&self, state: &quinn_udp::UdpState, cx: &mut Context, transmits: &[quinn_udp::Transmit]) -> Poll<io::Result<usize>> {
        let inner = self.arc.deref();
        let io = &inner.io;
        loop {
            ready!(io.poll_send_ready(cx))?;
            if let Ok(res) = io.try_io(Interest::WRITABLE, || inner.state.send(io.into(), state, transmits)) {
                return Poll::Ready(Ok(res));
            }
        }
    }

    fn poll_recv(
        &self,
        cx: &mut Context,
        bufs: &mut [std::io::IoSliceMut<'_>],
        meta: &mut [quinn_udp::RecvMeta],
    ) -> Poll<io::Result<usize>> {
        let inner = self.arc.deref();
        loop {
            ready!(inner.io.poll_recv_ready(cx))?;
            if let Ok(res) = inner
                .io
                .try_io(Interest::READABLE, || inner.state.recv((&inner.io).into(), bufs, meta))
            {
                return Poll::Ready(Ok(res));
            }
        }
    }

    fn local_addr(&self) -> io::Result<std::net::SocketAddr> {
        self.arc.io.local_addr()
    }

    fn may_fragment(&self) -> bool {
        quinn_udp::may_fragment()
    }
}
