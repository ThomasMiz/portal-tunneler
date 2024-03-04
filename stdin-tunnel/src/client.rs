use std::{net::SocketAddr, sync::Arc};

use quinn::{ClientConfig, Endpoint, EndpointConfig, IdleTimeout, RecvStream, SendStream, TokioRuntime, TransportConfig, VarInt};
use tokio::{
    io::{stdin, stdout},
    join,
    net::TcpListener,
};

use crate::{shared_socket::SharedUdpSocket, MAX_IDLE_TIMEOUT_MILLIS};

fn configure_client() -> ClientConfig {
    let crypto = rustls::ClientConfig::builder()
        .with_safe_defaults()
        .with_custom_certificate_verifier(SkipServerVerification::new())
        .with_no_client_auth();

    let mut client_config = ClientConfig::new(Arc::new(crypto));

    let mut transport_config = TransportConfig::default();
    transport_config.max_concurrent_uni_streams(0_u8.into());
    transport_config.max_idle_timeout(Some(IdleTimeout::from(VarInt::from_u32(MAX_IDLE_TIMEOUT_MILLIS))));
    client_config.transport_config(Arc::new(transport_config));

    client_config
}

fn make_client_endpoint(socket: SharedUdpSocket) -> Endpoint {
    let runtime = Arc::new(TokioRuntime);

    let mut endpoint = Endpoint::new_with_abstract_socket(EndpointConfig::default(), None, socket, runtime).unwrap();
    endpoint.set_default_client_config(configure_client());
    endpoint
}

pub async fn run_client(socket: SharedUdpSocket, server_addr: SocketAddr) {
    let local_addr = socket.local_addr().unwrap();
    let endpoint = make_client_endpoint(socket);

    let connection = loop {
        println!("Attempting to connect from {local_addr} to {server_addr}");
        let connecting = match endpoint.connect(server_addr, "localhost") {
            Ok(c) => c,
            Err(e) => {
                println!("Connect error: {e}");
                continue;
            }
        };

        println!("Connecting!");
        match connecting.await {
            Ok(c) => break c,
            Err(e) => println!("Connecting error: {e}"),
        }
    };

    println!("Client connected to {server_addr}");

    // IMPORTANT NOTE: QUIC streams are not received on the other end until actually used!
    let (send_stream, recv_stream) = match connection.open_bi().await {
        Ok(t) => t,
        Err(error) => {
            println!("Failed to open bidirectional stream: {error}");
            return;
        }
    };

    println!("Opened bidirectional stream {} {}", send_stream.id(), recv_stream.id());
    handle_bi_stream(send_stream, recv_stream).await;

    endpoint.close(VarInt::default(), b"Adios, fuckbois!");
    endpoint.wait_idle().await;
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

    /*let tcp_listener = TcpListener::bind("127.0.0.1:25565").await.unwrap();
    let (mut tcp_stream, _) = tcp_listener.accept().await.unwrap();
    let (mut recv_half, mut send_half) = tcp_stream.split();
    let (r1, r2) = join!(
        tokio::io::copy(&mut recv_stream, &mut send_half),
        tokio::io::copy(&mut recv_half, &mut send_stream),
    );

    println!("Finished stream:\nstream-to-stdout result: {r1:?}\nstdin-to-stream result: {r2:?}");*/
}

struct SkipServerVerification;

impl SkipServerVerification {
    fn new() -> Arc<Self> {
        Arc::new(Self)
    }
}

impl rustls::client::ServerCertVerifier for SkipServerVerification {
    fn verify_server_cert(
        &self,
        _end_entity: &rustls::Certificate,
        _intermediates: &[rustls::Certificate],
        _server_name: &rustls::ServerName,
        _scts: &mut dyn Iterator<Item = &[u8]>,
        _ocsp_response: &[u8],
        _now: std::time::SystemTime,
    ) -> Result<rustls::client::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::ServerCertVerified::assertion())
    }
}
