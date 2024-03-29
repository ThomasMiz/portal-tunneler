use std::{
    io::{self, ErrorKind},
    rc::Rc,
};

use quinn::{Connection, RecvStream, SendStream};
use tokio::{
    net::{TcpListener, TcpStream},
    try_join,
};

use crate::{
    tunnel_proto::{
        remote_tunnels::{
            OpenRemoteConnectionRequestRef, OpenRemoteConnectionResponse, StartRemoteTunnelRequest, StartRemoteTunnelResponseRef,
            TunnelTargetType,
        },
        serialize::{ByteRead, ByteWrite},
    },
    utils::bind_listeners,
};

pub async fn handle_start_remote_tunnels_stream(
    connection: Rc<Connection>,
    mut send_stream: SendStream,
    mut recv_stream: RecvStream,
) -> io::Result<()> {
    loop {
        let request = match StartRemoteTunnelRequest::read(&mut recv_stream).await {
            Ok(req) => req,
            Err(error) if error.kind() == ErrorKind::UnexpectedEof => return Ok(()),
            Err(error) => return Err(error),
        };

        let bind_result = bind_listeners(request.listen_at.as_ref()).await;
        let response = StartRemoteTunnelResponseRef::new(bind_result.as_ref().map(|_| ()));
        response.write(&mut send_stream).await?;

        if let Ok(listeners) = bind_result {
            let tunnel_id = request.tunnel_id;
            let target_type = request.target_type;
            for listener in listeners {
                let connection = Rc::clone(&connection);
                tokio::task::spawn_local(async move {
                    handle_remote_tunnel_listening(connection, listener, tunnel_id, target_type).await;
                });
            }
        }
    }
}

pub async fn handle_remote_tunnel_listening(
    connection: Rc<Connection>,
    listener: TcpListener,
    tunnel_id: u32,
    target_type: TunnelTargetType,
) {
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

pub async fn handle_remote_tunnel(
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
