use std::{
    net::{Ipv4Addr, SocketAddr, SocketAddrV4},
    rc::Rc,
    time::Duration,
};

use tokio::{net::UdpSocket, task::LocalSet};

fn get_forward_port(from_port: u16) -> Option<u16> {
    if from_port >= 5000 && from_port < 6000 {
        return Some(from_port + 1000);
    }

    if from_port >= 6000 && from_port < 7000 {
        return Some(from_port - 1000);
    }

    None
}

fn main() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    LocalSet::new().block_on(&runtime, async move {
        let port_start: u16 = 50500;
        let port_count: u16 = 5;

        let mut handles = Vec::with_capacity(port_count as usize);
        for i in 0..port_count {
            let addr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port_start + i));
            let socket = UdpSocket::bind(addr).await.unwrap();

            handles.push(tokio::task::spawn_local(async move {
                tuluburu(socket).await;
            }));
        }

        for handle in handles {
            let _ = handle.await;
        }
    });
}

async fn tuluburu(socket: UdpSocket) {
    let socket = Rc::new(socket);
    let mut buf = [0u8; 1400];
    let my_addr = socket.local_addr().unwrap();

    loop {
        let (size, from) = match socket.recv_from(&mut buf).await {
            Ok(t) => t,
            Err(error) => {
                println!("Socket {my_addr} error while receiving: {error}");
                continue;
            }
        };

        let forward_port = match get_forward_port(from.port()) {
            Some(p) => p,
            None => {
                println!("Socket {my_addr} received {size} bytes from {from}, not forwarding");
                continue;
            }
        };

        let socket = Rc::clone(&socket);
        tokio::task::spawn_local(async move {
            if rand::random::<u64>() % 1000 < 200 {
                println!("Packet dropped on purpose 😈");
                return;
            }

            tokio::time::sleep(Duration::from_millis(1000)).await;
            let dest = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, forward_port));
            match socket.send_to(&mut buf[..size], dest).await {
                Ok(sent) => {
                    println!("Socket {my_addr} forwarded {size} bytes from {from} to {sent} bytes to {dest}");
                }
                Err(error) => {
                    println!("Socket {my_addr} error while sending {size} bytes received from {from}: {error}");
                }
            };
        });
    }
}
