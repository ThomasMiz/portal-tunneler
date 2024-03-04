use std::{sync::Arc, time::Duration};

use quinn::{Connecting, Endpoint, EndpointConfig, IdleTimeout, RecvStream, SendStream, ServerConfig, TokioRuntime, VarInt};
use tokio::{
    io::{stdin, stdout},
    join,
    net::TcpSocket,
    select,
    task::AbortHandle,
};

use crate::{shared_socket::SharedUdpSocket, KEEPALIVE_INTERVAL_PERIOD_MILLIS, MAX_IDLE_TIMEOUT_MILLIS};

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

fn make_server_endpoint(socket: SharedUdpSocket) -> (Endpoint, Vec<u8>) {
    let runtime = Arc::new(TokioRuntime);
    let (server_config, server_cert) = configure_server();

    let endpoint = Endpoint::new_with_abstract_socket(EndpointConfig::default(), Some(server_config), socket, runtime).unwrap();
    (endpoint, server_cert)
}

pub async fn run_server(socket: SharedUdpSocket, mut abort_on_connect: Option<AbortHandle>) {
    println!("Starting server on {}", socket.local_addr().unwrap());
    let (endpoint, _server_cert) = make_server_endpoint(socket);

    loop {
        let incoming_connection = select! {
            biased;
            v = endpoint.accept() => v,
            //_ = tokio::signal::ctrl_c() => break, // TODO: Find out why Ctrl-C hangs instead ofclosingg
        };

        abort_on_connect.take().inspect(|handle| handle.abort());

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
    println!("Doing bidirectional copy");

    let mut stdout = stdout();
    let mut stdin = stdin();
    let (r1, r2) = join!(
        tokio::io::copy(&mut recv_stream, &mut stdout),
        tokio::io::copy(&mut stdin, &mut send_stream),
    );

    println!("Finished stream:\nstream-to-stdout result: {r1:?}\nstdin-to-stream result: {r2:?}");

    /*let mut tcp_stream = TcpSocket::new_v4().unwrap().connect("127.0.0.1:25565".parse().unwrap()).await.unwrap();
    let (mut recv_half, mut send_half) = tcp_stream.split();
    let (r1, r2) = join!(
        tokio::io::copy(&mut recv_stream, &mut send_half),
        tokio::io::copy(&mut recv_half, &mut send_stream),
    );

    println!("Finished stream:\nstream-to-stdout result: {r1:?}\nstdin-to-stream result: {r2:?}");*/
}
