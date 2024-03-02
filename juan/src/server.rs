use std::{
    net::{Ipv4Addr, SocketAddr, SocketAddrV4},
    sync::Arc,
    time::Duration,
};

use quinn::{Connecting, Endpoint, EndpointConfig, IdleTimeout, RecvStream, SendStream, ServerConfig, TokioRuntime, VarInt};
use tokio::select;

use crate::{KEEPALIVE_INTERVAL_PERIOD_MILLIS, MAX_IDLE_TIMEOUT_MILLIS, PORT};

fn configure_server() -> (ServerConfig, Vec<u8>) {
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
    let cert_der = cert.serialize_der().unwrap();
    let priv_key = rustls::PrivateKey(cert.serialize_private_key_der());
    let cert_chain = vec![rustls::Certificate(cert_der.clone())];

    let mut server_config = ServerConfig::with_single_cert(cert_chain, priv_key).unwrap();
    let transport_config = Arc::get_mut(&mut server_config.transport).unwrap();
    transport_config.max_concurrent_uni_streams(0_u8.into());
    transport_config.keep_alive_interval(Some(Duration::from_millis(KEEPALIVE_INTERVAL_PERIOD_MILLIS)));
    transport_config.max_idle_timeout(Some(IdleTimeout::from(VarInt::from_u32(MAX_IDLE_TIMEOUT_MILLIS))));

    (server_config, cert_der)
}

pub async fn make_server_endpoint(bind_addr: SocketAddr) -> (Endpoint, Vec<u8>) {
    let runtime = Arc::new(TokioRuntime);

    let socket = tokio::net::UdpSocket::bind(bind_addr).await.unwrap();
    let socket = socket.into_std().unwrap();

    let (server_config, server_cert) = configure_server();

    let endpoint = Endpoint::new(EndpointConfig::default(), Some(server_config), socket, runtime).unwrap();
    (endpoint, server_cert)
}

pub async fn run_server() {
    let addr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, PORT));
    let (endpoint, _server_cert) = make_server_endpoint(addr).await;

    loop {
        let incoming_connection = select! {
            biased;
            v = endpoint.accept() => v,
            _ = tokio::signal::ctrl_c() => break,
        };

        let incoming_connection = match incoming_connection {
            Some(c) => c,
            None => break,
        };

        println!("Incoming connection form addr={}", incoming_connection.remote_address());
        tokio::task::spawn_local(async move {
            handle_connection(incoming_connection).await;
        });
    }

    endpoint.close(VarInt::from_u32(69), b"Server is shutting down");
    println!("Server closed");
}

async fn handle_connection(incoming_connection: Connecting) {
    let connection = match incoming_connection.await {
        Ok(c) => c,
        Err(connection_error) => {
            println!("Failed to accept incoming connection: {connection_error}");
            return;
        }
    };

    loop {
        let (send_stream, recv_stream) = match connection.accept_bi().await {
            Ok(v) => v,
            Err(error) => {
                println!("Failed to accept bidirectional stream: {error}");
                break;
            }
        };

        println!("Accepted bidirectional stream {} {}", send_stream.id(), recv_stream.id());
        tokio::task::spawn_local(async move {
            handle_bi_stream(send_stream, recv_stream).await;
        });
    }
}

async fn handle_bi_stream(mut send_stream: SendStream, mut recv_stream: RecvStream) {
    let mut buf = [0u8; 0x2000];

    loop {
        let bytes_read = match recv_stream.read(&mut buf).await {
            Ok(Some(v)) => v,
            Ok(None) => {
                println!(
                    "Stream {} closed, shutting down related send stream {}",
                    recv_stream.id(),
                    send_stream.id()
                );
                if let Err(write_error) = send_stream.finish().await {
                    println!("Failed to gracefully finish send stream {}: {write_error}", send_stream.id());
                }

                break;
            }
            Err(read_error) => {
                println!("Stream {} rejected read with error {read_error}", recv_stream.id());
                break;
            }
        };

        for ele in &mut buf[..bytes_read] {
            *ele = ele.to_ascii_lowercase();
        }

        if let Err(write_error) = send_stream.write_all(&buf[..bytes_read]).await {
            println!("Stream {} rejected write with error: {write_error}", send_stream.id());
            break;
        }
    }
}
