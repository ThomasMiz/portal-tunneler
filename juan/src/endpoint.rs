use std::{io, sync::Arc, time::Duration};

use quinn::{ClientConfig, Endpoint, EndpointConfig, IdleTimeout, ServerConfig, TokioRuntime, TransportConfig, VarInt};

use crate::shared_socket::SharedUdpSocket;

pub const KEEPALIVE_INTERVAL_PERIOD_MILLIS: u64 = 1000;
pub const MAX_IDLE_TIMEOUT_MILLIS: u32 = 4000;

pub enum EndpointSocketSource {
    Simple(std::net::UdpSocket),
    Shared(SharedUdpSocket),
}

pub fn make_endpoint(socket: EndpointSocketSource, is_client: bool, is_server: bool) -> io::Result<Endpoint> {
    let runtime = Arc::new(TokioRuntime);

    let client_config = match is_client {
        true => Some(configure_client()),
        false => None,
    };

    let server_config = match is_server {
        true => Some(configure_server().0),
        false => None,
    };

    let mut endpoint = match socket {
        EndpointSocketSource::Simple(socket) => Endpoint::new(EndpointConfig::default(), server_config, socket, runtime)?,
        EndpointSocketSource::Shared(socket) => {
            Endpoint::new_with_abstract_socket(EndpointConfig::default(), server_config, socket, runtime)?
        }
    };

    if let Some(client_config) = client_config {
        endpoint.set_default_client_config(client_config);
    }

    Ok(endpoint)
}

pub fn configure_client() -> ClientConfig {
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

pub fn configure_server() -> (ServerConfig, Vec<u8>) {
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
