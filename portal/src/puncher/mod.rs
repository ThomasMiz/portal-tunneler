use std::{
    io::{Error, ErrorKind},
    net::{IpAddr, SocketAddr},
    num::NonZeroU16,
    time::Duration,
};

use portal_puncher_sm as sm;

use tokio::{net::UdpSocket, select, task::JoinHandle};

use crate::{
    shared_socket::SharedUdpSocket,
    utils::{recv_from_any, sleep_until_if_some},
};

pub mod connection_code;
pub mod get_public_ip;
pub mod socket_binder;

pub enum PunchConnectResult {
    Connect(UdpSocket, SocketAddr),
    Listen(SharedUdpSocket, SocketAddr, JoinHandle<()>),
}

pub async fn punch_connection(
    is_server: bool,
    mut sockets: Vec<UdpSocket>,
    remote_address: IpAddr,
    remote_port_start: NonZeroU16,
    lane_count: NonZeroU16,
) -> Result<PunchConnectResult, Error> {
    let port_start = NonZeroU16::new(sockets[0].local_addr().unwrap().port()).unwrap();

    let mut puncher = sm::Puncher::new(
        is_server,
        port_start,
        remote_address,
        remote_port_start,
        lane_count,
        Duration::from_millis(1500),
        Duration::from_secs(20),
    );

    let mut buf = [0u8; sm::MAX_REASONABLE_PAYLOAD];
    let mut packet_counter = 0u32;

    println!("Entering loop");
    let ports = loop {
        while let Some(send_info) = puncher.send_to(&mut buf, &packet_counter.to_le_bytes()) {
            println!(
                "Sending {} bytes from port {} to {} with counter {packet_counter}",
                send_info.length, send_info.from_port, send_info.to
            );
            let index = (send_info.from_port.get() - port_start.get()) as usize;
            let send_result = sockets[index].send_to(&buf[..send_info.length], send_info.to).await;
            println!(
                "Sent {} bytes from {} to {}",
                send_info.length,
                sockets[index].local_addr().unwrap(),
                send_info.to
            );
            if let Err(error) = send_result {
                println!("Send failed!");
                puncher.send_failed(send_info.from_port.get(), error);
            }
            packet_counter += 1;
        }

        select! {
            biased;
            (index, result) = recv_from_any(&sockets, &mut buf) => {
                println!("Received packet from port {}: {result:?}", port_start.get() + index as u16);
                let result = result.map(|(len, addr)| (&buf[..len], addr));

                let maybe_application_data = puncher.received_from(result, port_start.get() + index as u16);

                if let Some(application_data) = maybe_application_data {
                    if application_data.len() == 4 {
                        let counter = u32::from_le_bytes(*application_data.first_chunk().unwrap());
                        println!("Application data with counter {counter}");
                    } else {
                        println!("Unknown application data with length {}", application_data.len());
                    }
                } else {
                    println!("(No application data)");
                }
            }
            _ = sleep_until_if_some(puncher.next_tick_instant()) => {
                println!("Ticking");
                puncher.tick();
            }
        }

        let action = puncher.poll();
        println!("Puncher polled: {action:?}");

        match action {
            sm::PuncherAction::Wait => {}
            sm::PuncherAction::Connect(ports) => break ports,
            sm::PuncherAction::Listen(ports) => break ports,
            sm::PuncherAction::Failed => {
                return Err(Error::new(ErrorKind::Other, "Failed to establish a connection on any lane"));
            }
            sm::PuncherAction::Timeout => {
                return Err(Error::new(ErrorKind::Other, "Failed to establish a connection before the timeout"));
            }
            sm::PuncherAction::ClientServerMismatch => {
                return Err(Error::new(
                    ErrorKind::Other,
                    "Client-server mismatch. Make sure one host is server and the other is client",
                ));
            }
        }
    };

    let socket_index = (ports.local.get() - port_start.get()) as usize;
    let socket = sockets.swap_remove(socket_index);
    drop(sockets);

    println!("Selected socket index {socket_index} addr {}", socket.local_addr().unwrap());

    let remote_address = SocketAddr::new(remote_address, ports.remote.get());
    let result = match is_server {
        true => {
            let socket = SharedUdpSocket::new(socket).unwrap();
            let socket2 = SharedUdpSocket::clone(&socket);
            let handle = tokio::task::spawn_local(async move {
                server_background_task(socket2, puncher, packet_counter).await;
            });

            PunchConnectResult::Listen(socket, remote_address, handle)
        }
        false => PunchConnectResult::Connect(socket, remote_address),
    };

    Ok(result)
}

async fn server_background_task(socket: SharedUdpSocket, mut puncher: sm::Puncher, mut packet_counter: u32) {
    println!("Started background task to keep sending packets");
    let mut buf = [0u8; sm::MAX_REASONABLE_PAYLOAD];
    loop {
        println!("Another background tick");
        puncher.tick();
        while let Some(send_info) = puncher.send_to(&mut buf, &packet_counter.to_le_bytes()) {
            println!(
                "Sending {} bytes from port {} to {} with counter {packet_counter}",
                send_info.length, send_info.from_port, send_info.to
            );
            let send_result = socket.send_to(&buf[..send_info.length], send_info.to).await;
            println!(
                "Sent {} bytes from {} to {}",
                send_info.length,
                socket.local_addr().unwrap(),
                send_info.to
            );
            if let Err(error) = send_result {
                println!("Send failed!");
                puncher.send_failed(send_info.from_port.get(), error);
            }
            packet_counter += 1;
        }

        sleep_until_if_some(puncher.next_tick_instant()).await;
    }
}
