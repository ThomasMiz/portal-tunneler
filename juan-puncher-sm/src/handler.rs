use std::{io, net::SocketAddr};

pub trait PuncherHandler {
    fn send_to(&mut self, source_port: u16, buf: &[u8], target: SocketAddr) -> io::Result<usize>;
}
