use std::{
    io::Error,
    net::{Ipv4Addr, SocketAddr, SocketAddrV4},
    sync::Arc,
};

use quinn::{Endpoint, ServerConfig};

use crate::PORT;

fn configure_server() -> (ServerConfig, Vec<u8>) {
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
    let cert_der = cert.serialize_der().unwrap();
    let priv_key = cert.serialize_private_key_der();
    let priv_key = rustls::PrivateKey(priv_key);
    let cert_chain = vec![rustls::Certificate(cert_der.clone())];

    let mut server_config = ServerConfig::with_single_cert(cert_chain, priv_key).unwrap();
    let transport_config = Arc::get_mut(&mut server_config.transport).unwrap();
    transport_config.max_concurrent_uni_streams(0_u8.into());

    (server_config, cert_der)
}

pub fn make_server_endpoint(bind_addr: SocketAddr) -> (Endpoint, Vec<u8>) {
    let (server_config, server_cert) = configure_server();
    let endpoint = Endpoint::server(server_config, bind_addr).unwrap();
    (endpoint, server_cert)
}

pub async fn run_server() -> Result<(), Error> {
    let addr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, PORT));
    let (endpoint, _server_cert) = make_server_endpoint(addr);
    // accept a single connection
    let incoming_conn = endpoint.accept().await.unwrap();
    let conn = incoming_conn.await.unwrap();
    println!("Server connection accepted: addr={}", conn.remote_address());
    Ok(())
}
