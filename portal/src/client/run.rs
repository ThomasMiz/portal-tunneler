use std::{io, rc::Rc};

use portal_tunneler_proto::shared::tunnels::TunnelSide;
use quinn::{Connection, ConnectionError};

use crate::{
    args::StartClientConfig,
    client::{
        local_tunnels::handle_local_tunnel_listening,
        remote_tunnels::{create_remote_tunnels, handle_incoming_bi_stream},
    },
    utils::bind_listeners,
};

pub async fn run_client(connection: Connection, config: StartClientConfig) -> io::Result<()> {
    println!("Client connected to {}", connection.remote_address());

    let connection = Rc::new(connection);
    let mut tunnels = config.tunnels;

    for spec in tunnels.extract_if(|spec| spec.side == TunnelSide::Local) {
        match bind_listeners(spec.listen_address.as_ref()).await {
            Ok(listeners) => {
                let spec = Rc::new(spec);
                for listener in listeners {
                    let connection = Rc::clone(&connection);
                    let spec = Rc::clone(&spec);
                    tokio::task::spawn_local(async move {
                        handle_local_tunnel_listening(connection, listener, spec).await;
                    });
                }
            }
            Err(error) => {
                eprintln!("Couldn't open tunnel {}: {error}", spec.index);
            }
        }
    }

    let remote_tunnels = Rc::new(create_remote_tunnels(&connection, tunnels).await?);

    let result_error = loop {
        let (send_stream, recv_stream) = match connection.accept_bi().await {
            Ok(t) => t,
            Err(error) => break error,
        };

        let remote_tunnels = Rc::clone(&remote_tunnels);
        tokio::task::spawn_local(async move {
            match handle_incoming_bi_stream(send_stream, recv_stream, remote_tunnels).await {
                Ok(()) => {}
                Err(error) => println!("Handle incoming bidi stream task finished with error: {error}"),
            }
        });
    };

    match result_error {
        ConnectionError::LocallyClosed => {}
        ConnectionError::ApplicationClosed(_) => println!("The server closed the connection"),
        error => eprintln!("The connection closed unexpectedly: {error}"),
    };

    Ok(())
}
