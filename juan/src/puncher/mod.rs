use std::{
    io::{self, Error, ErrorKind},
    net::{IpAddr, SocketAddr},
    num::NonZeroU16,
};

use tokio::net::UdpSocket;

use self::{connection_code::ConnectionCode, socket_binder::bind_sockets};

pub mod connection_code;
pub mod get_public_ip;
pub mod socket_binder;

#[derive(Debug)]
pub struct PuncherStarter {
    sockets: Vec<std::net::UdpSocket>,
}

impl PuncherStarter {
    pub fn new(bind_address: SocketAddr, lane_count: NonZeroU16) -> io::Result<Self> {
        Ok(Self {
            sockets: bind_sockets(bind_address, lane_count)?,
        })
    }

    pub fn generate_connection_code(&self, public_address: IpAddr) -> ConnectionCode {
        let first_address = self.sockets.first().unwrap().local_addr().unwrap();
        ConnectionCode::new(
            public_address,
            first_address.port(),
            NonZeroU16::new(self.sockets.len() as u16).unwrap(),
        )
    }

    pub fn set_remote(self, remote_address: IpAddr, port_start: u16, lane_count: NonZeroU16) -> io::Result<Puncher> {
        let (_, overflows) = port_start.overflowing_add(lane_count.get());
        if overflows {
            return Err(Error::new(ErrorKind::InvalidData, "The lane count overflows the port number"));
        }

        let mut lanes = Vec::with_capacity(self.sockets.len());

        let mut target_port = port_start;
        for socket in self.sockets {
            lanes.push(PunchLane {
                socket: tokio::net::UdpSocket::from_std(socket)?,
                target_port,
                status: PunchLaneStatus::Connecting,
            });

            target_port += 1;
        }

        Ok(Puncher { remote_address, lanes })
    }
}

#[derive(Debug)]
pub struct Puncher {
    remote_address: IpAddr,
    lanes: Vec<PunchLane>,
}

#[derive(Debug)]
pub struct PunchLane {
    socket: UdpSocket,
    target_port: u16,
    status: PunchLaneStatus,
}

#[derive(Debug)]
pub enum PunchLaneStatus {
    Connecting,
    Connected(u16),
    Occupied,
}

impl Puncher {}
