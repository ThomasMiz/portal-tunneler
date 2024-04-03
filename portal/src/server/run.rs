use std::{io, rc::Rc};

use portal_tunneler_proto::{serialize::ByteRead, shared::ClientStreamRequest};
use quinn::{Connecting, Connection, Endpoint, RecvStream, SendStream, VarInt};
use tokio::{
    select,
    task::{AbortHandle, JoinHandle},
};

use super::{local_tunnels::handle_open_local_tunnel_stream, remote_tunnels::handle_start_remote_tunnels_stream};

pub async fn run_server(endpoint: Endpoint, abort_on_connect: Option<JoinHandle<()>>) {
    println!("Starting server on {}", endpoint.local_addr().unwrap());

    loop {
        println!("Waiting for next incoming connection");
        let incoming_connection = select! {
            biased;
            v = endpoint.accept() => v,
            //_ = tokio::signal::ctrl_c() => break, // TODO: Find out why Ctrl-C hangs instead of closing
        };

        let incoming_connection = match incoming_connection {
            Some(c) => c,
            None => break,
        };

        let abort_on_connect = abort_on_connect.as_ref().map(|h| h.abort_handle());
        println!("Incoming connection form addr={}", incoming_connection.remote_address());
        tokio::task::spawn_local(async move {
            handle_connection(incoming_connection, abort_on_connect).await;
        });
    }

    endpoint.close(VarInt::from_u32(69), b"Server is shutting down");
    println!("Server closed");
}

async fn handle_connection(incoming_connection: Connecting, abort_on_connect: Option<AbortHandle>) {
    let connection = match incoming_connection.await {
        Ok(c) => c,
        Err(connection_error) => {
            println!("Failed to accept incoming connection: {connection_error}");
            return;
        }
    };

    abort_on_connect.inspect(|h| h.abort());
    let connection = Rc::new(connection);

    loop {
        let (send_stream, recv_stream) = match connection.accept_bi().await {
            Ok(v) => v,
            Err(error) => {
                println!("Failed to accept bidirectional stream: {error}");
                break;
            }
        };

        println!("Accepted bidirectional stream {} {}", send_stream.id(), recv_stream.id());
        let connection = Rc::clone(&connection);
        tokio::task::spawn_local(async move {
            match handle_incoming_bi_stream(connection, send_stream, recv_stream).await {
                Ok(()) => {}
                Err(error) => println!("Handle bidi stream finished with error: {error}"),
            }
        });
    }
}

async fn handle_incoming_bi_stream(connection: Rc<Connection>, send_stream: SendStream, mut recv_stream: RecvStream) -> io::Result<()> {
    let request = ClientStreamRequest::read(&mut recv_stream).await?;
    match request {
        ClientStreamRequest::OpenLocalTunnelConnection => handle_open_local_tunnel_stream(send_stream, recv_stream).await,
        ClientStreamRequest::StartRemoteTunnels => handle_start_remote_tunnels_stream(connection, send_stream, recv_stream).await,
    }
}
