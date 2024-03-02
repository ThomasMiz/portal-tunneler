use std::{
    net::{Ipv4Addr, SocketAddr, SocketAddrV4},
    sync::Arc,
};

use quinn::{ClientConfig, Endpoint, IdleTimeout, TransportConfig, VarInt};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    select,
};

use crate::{MAX_IDLE_TIMEOUT_MILLIS, PORT};

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

pub async fn run_client() {
    let server_addr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, PORT));

    let local_addr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0));
    let mut endpoint = Endpoint::client(local_addr).unwrap();
    endpoint.set_default_client_config(configure_client());

    let connection = endpoint.connect(server_addr, "localhost").unwrap().await.unwrap();
    println!("Client connected from {local_addr} to {server_addr}");

    let (mut send_stream, mut recv_stream) = match connection.open_bi().await {
        Ok(t) => t,
        Err(error) => {
            println!("Failed to open bidirectional stream: {error}");
            return;
        }
    };

    println!("Opened bidirectional stream {} {}", send_stream.id(), recv_stream.id());

    let mut recv_buf = [0u8; 0x2000];
    let mut stdin_buf = [0u8; 0x2000];
    let mut stdin = tokio::io::stdin();

    loop {
        select! {
            biased;
            read_result = recv_stream.read(&mut recv_buf) => {
                let bytes_read = match read_result {
                    Ok(Some(v)) => v,
                    Ok(None) => {
                        println!("Recv stream closed prematurely");
                        break;
                    }
                    Err(error) => {
                        println!("Read from recv stream failed: {error}");
                        break;
                    },
                };

                if let Err(error) = tokio::io::stdout().write_all(&recv_buf[..bytes_read]).await {
                    println!("Write to stdout failed: {error}");
                    break;
                }
            }
            read_result = stdin.read(&mut stdin_buf) => {
                let bytes_read = match read_result {
                    Ok(v) => v,
                    Err(error) => {
                        println!("Read from stdin failed: {error}");
                        break;
                    },
                };

                if let Err(error) = send_stream.write_all(&stdin_buf[..bytes_read]).await {
                    println!("Write to send stream failed: {error}");
                    break;
                }
            }
        }
    }

    endpoint.close(VarInt::default(), b"Adios, fuckbois!");
    endpoint.wait_idle().await;
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
