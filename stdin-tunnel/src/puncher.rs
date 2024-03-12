use std::{
    io::{Error, ErrorKind},
    net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4},
    num::NonZeroU16,
    time::Duration,
};

use portal_puncher_sm as sm;

use tokio::{select, task::JoinHandle};

use crate::{
    shared_socket::SharedUdpSocket,
    utils::{recv_from_any, sleep_until_if_some},
};

pub async fn punch(
    is_server: bool,
    port_start: NonZeroU16,
    remote_address: IpAddr,
    remote_port_start: NonZeroU16,
    lane_count: NonZeroU16,
) -> Result<(SharedUdpSocket, Option<JoinHandle<()>>, SocketAddr), Error> {
    let mut puncher = sm::Puncher::new(
        is_server,
        port_start,
        remote_address,
        remote_port_start,
        lane_count,
        Duration::from_millis(1500),
        Duration::from_secs(20),
    );

    let mut sockets = Vec::with_capacity(lane_count.get() as usize);
    let port_start = port_start.get();
    let port_end = port_start + lane_count.get();

    println!("Binding sockets");
    for port in port_start..port_end {
        let bind_addr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, port));
        let std_socket = std::net::UdpSocket::bind(bind_addr).unwrap();
        std_socket.set_nonblocking(true).unwrap();
        sockets.push(tokio::net::UdpSocket::from_std(std_socket).unwrap());
    }

    let mut buf = [0u8; sm::MAX_REASONABLE_PAYLOAD];
    let mut packet_counter = 0u32;

    println!("Entering loop");
    let ports = loop {
        while let Some(send_info) = puncher.send_to(&mut buf, &packet_counter.to_le_bytes()) {
            println!(
                "Sending {} bytes from port {} to {} with counter {packet_counter}",
                send_info.length, send_info.from_port, send_info.to
            );
            let index = (send_info.from_port.get() - port_start) as usize;
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
                println!("Received packet from port {}: {result:?}", port_start + index as u16);
                let result = result.map(|(len, addr)| (&buf[..len], addr));

                let maybe_application_data = puncher.received_from(result, port_start + index as u16);

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

    let socket_index = (ports.local.get() - port_start) as usize;
    let socket = sockets.swap_remove(socket_index);
    drop(sockets);
    println!("Selected socket index {socket_index} addr {}", socket.local_addr().unwrap());

    let socket = SharedUdpSocket::new(socket).unwrap();
    let socket2 = SharedUdpSocket::clone(&socket);

    let handle = match is_server {
        false => None,
        true => Some(tokio::task::spawn_local(async move {
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
                    let send_result = socket2.send_to(&buf[..send_info.length], send_info.to).await;
                    println!(
                        "Sent {} bytes from {} to {}",
                        send_info.length,
                        socket2.local_addr().unwrap(),
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
        })),
    };

    let remote_address = SocketAddr::new(remote_address, ports.remote.get());
    Ok((socket, handle, remote_address))
}
