use std::{io::Error, net::{Ipv4Addr, SocketAddr, SocketAddrV4}, sync::Arc};

use quinn::{ClientConfig, Endpoint};

use crate::PORT;

pub async fn run_client() -> Result<(), Error> {
    let server_addr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, PORT));

    let local_addr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0));
    let mut endpoint = Endpoint::client(local_addr).unwrap();
    endpoint.set_default_client_config(configure_client());

    let connection = endpoint.connect(server_addr, "localhost").unwrap().await.unwrap();
    println!("Client connected from {local_addr} to {server_addr}");
    drop(connection);

    endpoint.wait_idle().await;

    Ok(())
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

fn configure_client() -> ClientConfig {
    let crypto = rustls::ClientConfig::builder()
        .with_safe_defaults()
        .with_custom_certificate_verifier(SkipServerVerification::new())
        .with_no_client_auth();

    ClientConfig::new(Arc::new(crypto))
}