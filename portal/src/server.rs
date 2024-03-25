use std::{
    io::{self, ErrorKind},
    rc::Rc,
};

use quinn::{Connecting, Connection, Endpoint, RecvStream, SendStream, VarInt};
use tokio::{
    net::{TcpListener, TcpStream},
    select,
    task::{AbortHandle, JoinHandle},
    try_join,
};

use crate::{
    tunnel_proto::{
        local_tunnels::{OpenLocalConnectionRequest, OpenLocalConnectionResponseRef},
        remote_tunnels::{
            OpenRemoteConnectionRequestRef, OpenRemoteConnectionResponse, StartRemoteTunnelRequest, StartRemoteTunnelResponseRef,
            TunnelTargetType,
        },
        requests::ClientStreamRequest,
        responses::OpenConnectionError,
        serialize::{ByteRead, ByteWrite},
    },
    utils::UNSPECIFIED_SOCKADDR_V4,
};

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

async fn handle_open_local_tunnel_stream(mut send_stream: SendStream, mut recv_stream: RecvStream) -> io::Result<()> {
    println!("Incoming connection from on tunnel");

    let mut request = OpenLocalConnectionRequest::read(&mut recv_stream).await?;
    println!("Connecting connection from remote tunnel to {}", request.target);

    let tcp_stream_result = request.target.bind_connect().await;

    let response_result = tcp_stream_result
        .as_ref()
        .map(|stream| stream.local_addr().unwrap_or(UNSPECIFIED_SOCKADDR_V4))
        .map_err(|error| {
            (OpenConnectionError::Connect, error) // TODO: Proper OpenConnectionError value
        });

    match response_result {
        Ok(bind_address) => println!(
            "Local tunnel connected to {} (local socket bound at {bind_address})",
            request.target
        ),
        Err((start_error, error)) => eprintln!("Local tunnel failed to connect to target due to {start_error} failure: {error}"),
    }

    OpenLocalConnectionResponseRef::new(response_result).write(&mut send_stream).await?;

    let mut tcp_stream = tcp_stream_result?;
    let (mut read_half, mut write_half) = tcp_stream.split();
    let result = try_join!(
        tokio::io::copy(&mut read_half, &mut send_stream),
        tokio::io::copy(&mut recv_stream, &mut write_half),
    );

    match result {
        Ok((sent, received)) => {
            println!("Local tunnel ended after {sent} bytes sent and {received} bytes received");
            Ok(())
        }
        Err(error) => {
            eprintln!("Local tunnel ended with error: {error}");
            Err(error)
        }
    }
}

async fn handle_start_remote_tunnels_stream(
    connection: Rc<Connection>,
    mut send_stream: SendStream,
    mut recv_stream: RecvStream,
) -> io::Result<()> {
    loop {
        let mut request = match StartRemoteTunnelRequest::read(&mut recv_stream).await {
            Ok(req) => req,
            Err(error) if error.kind() == ErrorKind::UnexpectedEof => return Ok(()),
            Err(error) => return Err(error),
        };

        let bind_result = request.listen_at.bind_listener().await;
        let response = StartRemoteTunnelResponseRef::new(bind_result.as_ref().map(|_| ()));
        response.write(&mut send_stream).await?;

        if let Ok(listener) = bind_result {
            let connection = Rc::clone(&connection);
            let tunnel_id = request.tunnel_id;
            let target_type = request.target_type;
            tokio::task::spawn_local(async move {
                handle_remote_tunnel_listening(connection, listener, tunnel_id, target_type).await;
            });
        }
    }
}

async fn handle_remote_tunnel_listening(connection: Rc<Connection>, listener: TcpListener, tunnel_id: u32, target_type: TunnelTargetType) {
    loop {
        let (tcp_stream, _from) = match listener.accept().await {
            Ok(t) => t,
            Err(error) => {
                eprintln!("Error accepting new incoming connection: {error}");
                continue;
            }
        };

        let connection = Rc::clone(&connection);
        tokio::task::spawn_local(async move {
            match handle_remote_tunnel(connection, tcp_stream, tunnel_id, target_type).await {
                Ok(()) => {}
                Err(error) => println!("Remote tunnel task finished with error: {error}"),
            }
        });
    }
}

async fn handle_remote_tunnel(
    connection: Rc<Connection>,
    mut tcp_stream: TcpStream,
    tunnel_id: u32,
    target_type: TunnelTargetType,
) -> io::Result<()> {
    let (mut send_stream, mut recv_stream) = match connection.open_bi().await {
        Ok(t) => t,
        Err(error) => {
            eprintln!("Couldn't start remote tunnel, error while opening bidi stream: {error}");
            return Err(error.into());
        }
    };

    let maybe_target = match target_type {
        TunnelTargetType::Static => {
            println!("Tunneling through static tunnel");
            None
        }
        TunnelTargetType::Socks => {
            unimplemented!("TODO: Implement SOCKS protocol")
        }
    };

    let request = OpenRemoteConnectionRequestRef::new(tunnel_id, maybe_target);
    request.write(&mut send_stream).await?;

    let response = OpenRemoteConnectionResponse::read(&mut recv_stream).await?;
    let bound_address = match response.result {
        Ok(addr) => addr,
        Err((start_error, error)) => {
            eprintln!("Remote tunnel failed to connect to target due to {start_error} failure: {error}");
            return Err(error);
        }
    };

    println!("Remote tunnel connected (remote socket bound at {bound_address})");
    let (mut read_half, mut write_half) = tcp_stream.split();
    let result = try_join!(
        tokio::io::copy(&mut read_half, &mut send_stream),
        tokio::io::copy(&mut recv_stream, &mut write_half),
    );

    match result {
        Ok((sent, received)) => {
            println!("Remote tunnel ended after {sent} bytes sent and {received} bytes received");
            Ok(())
        }
        Err(error) => {
            eprintln!("Remote tunnel ended with error: {error}");
            Err(error)
        }
    }
}
