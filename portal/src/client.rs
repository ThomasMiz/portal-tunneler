use std::{
    collections::HashMap,
    io::{self, Error, ErrorKind},
    rc::Rc,
};

use quinn::{Connection, ConnectionError, RecvStream, SendStream};
use tokio::{
    net::{TcpListener, TcpStream},
    try_join,
};

use crate::{
    args::{TunnelSide, TunnelSpec, TunnelTarget},
    tunnel_proto::{
        local_tunnels::{OpenLocalConnectionRequestRef, OpenLocalConnectionResponse},
        remote_tunnels::{
            OpenRemoteConnectionRequest, OpenRemoteConnectionResponseRef, StartRemoteTunnelRequestRef, StartRemoteTunnelResponse,
            TunnelTargetType,
        },
        requests::ClientStreamRequest,
        responses::OpenConnectionError,
        serialize::{ByteRead, ByteWrite},
    },
    utils::{bind_connect, bind_listeners, UNSPECIFIED_SOCKADDR_V4},
};

pub async fn run_client(connection: Connection, tunnels: Vec<TunnelSpec>) -> io::Result<()> {
    println!("Client connected to {}", connection.remote_address());

    let connection = Rc::new(connection);

    let mut remote_tunnel_specs = Vec::new();

    for spec in tunnels.into_iter() {
        if spec.side == TunnelSide::Remote {
            remote_tunnel_specs.push(spec);
            continue;
        }

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

    let remote_tunnels = Rc::new(create_remote_tunnels(&connection, remote_tunnel_specs).await?);

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

async fn create_remote_tunnels(connection: &Connection, remote_tunnel_specs: Vec<TunnelSpec>) -> io::Result<HashMap<u32, TunnelSpec>> {
    if remote_tunnel_specs.is_empty() {
        return Ok(HashMap::new());
    }

    let (mut send_stream, mut recv_stream) = connection.open_bi().await?;
    ClientStreamRequest::StartRemoteTunnels.write(&mut send_stream).await?;

    async fn send_tunnel_specs_task(send_stream: &mut SendStream, remote_tunnel_specs: &[TunnelSpec]) -> io::Result<()> {
        for (i, spec) in remote_tunnel_specs.iter().enumerate() {
            let target_type = match spec.target {
                TunnelTarget::Address(_) => TunnelTargetType::Static,
                TunnelTarget::Socks => TunnelTargetType::Socks,
            };

            let request = StartRemoteTunnelRequestRef::new(i as u32, target_type, spec.listen_address.as_ref());
            request.write(send_stream).await?;
        }

        send_stream.finish().await?;
        Ok(())
    }

    async fn receive_tunnel_results(
        recv_stream: &mut RecvStream,
        remote_tunnel_specs: &[TunnelSpec],
    ) -> io::Result<HashMap<u32, TunnelSpec>> {
        let mut map = HashMap::new();

        for (i, spec) in remote_tunnel_specs.iter().enumerate() {
            let response = StartRemoteTunnelResponse::read(recv_stream).await?;
            match response.result {
                Ok(()) => {
                    map.insert(i as u32, spec.clone()); // <-- TODO: This clone could be avoided
                }
                Err(error) => eprintln!("Couldn't start remote tunnel {}, server responded with error: {error}", spec.index),
            }
        }

        Ok(map)
    }

    let (_, map) = try_join!(
        send_tunnel_specs_task(&mut send_stream, &remote_tunnel_specs),
        receive_tunnel_results(&mut recv_stream, &remote_tunnel_specs),
    )?;

    Ok(map)
}

async fn handle_local_tunnel_listening(connection: Rc<Connection>, listener: TcpListener, spec: Rc<TunnelSpec>) {
    loop {
        let (tcp_stream, from) = match listener.accept().await {
            Ok(t) => t,
            Err(error) => {
                eprintln!("Error accepting new incoming connection: {error}");
                continue;
            }
        };

        print!("Incoming connection into tunnel {} from {from}, ", spec.index);
        match &spec.target {
            TunnelTarget::Socks => println!("waiting for SOCKS command"),
            TunnelTarget::Address(address) => println!("tunneling towards {address}"),
        };

        let connection = Rc::clone(&connection);
        let spec = Rc::clone(&spec);
        tokio::task::spawn_local(async move {
            match handle_local_tunnel(connection, tcp_stream, spec).await {
                Ok(()) => {}
                Err(error) => println!("Local tunnel task finished with error: {error}"),
            }
        });
    }
}

async fn handle_local_tunnel(connection: Rc<Connection>, mut tcp_stream: TcpStream, spec: Rc<TunnelSpec>) -> io::Result<()> {
    let (mut send_stream, mut recv_stream) = connection.open_bi().await?;
    ClientStreamRequest::OpenLocalTunnelConnection.write(&mut send_stream).await?;

    let request = match &spec.target {
        TunnelTarget::Socks => unimplemented!("TODO: Implement SOCKS protocol"),
        TunnelTarget::Address(address) => OpenLocalConnectionRequestRef::new(address.as_ref()),
    };

    request.write(&mut send_stream).await?;

    let response = OpenLocalConnectionResponse::read(&mut recv_stream).await?;
    let bind_address = match response.result {
        Ok(address) => address,
        Err((start_error, error)) => {
            eprintln!("Failed to connect local tunnel, server responded with {start_error} failure: {error}");
            return Err(error);
        }
    };

    println!("Local tunnel connected through server (remote socket bound at {bind_address})");

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

async fn handle_incoming_bi_stream(
    mut send_stream: SendStream,
    mut recv_stream: RecvStream,
    remote_tunnels: Rc<HashMap<u32, TunnelSpec>>,
) -> io::Result<()> {
    // Incoming (server-opened) bidi streams are exclusively used for new connections in a remote tunnel.

    println!("Incoming connection from remote tunnel");

    let request = OpenRemoteConnectionRequest::read(&mut recv_stream).await?;

    let spec = match remote_tunnels.get(&request.tunnel_id) {
        Some(spec) => spec,
        None => {
            eprintln!("Error: Server opened a new tunnel but specified invalid tunnel ID");
            return Err(Error::new(ErrorKind::Other, "Server specified invalid tunnel ID"));
        }
    };

    let address = match &spec.target {
        TunnelTarget::Socks => {
            let target_address = match request.maybe_target {
                Some(addr) => addr,
                None => {
                    eprintln!("The server specified an address as target on a static tunnel");
                    return Err(Error::new(
                        ErrorKind::Other,
                        "The server specified an address as target on a static tunnel",
                    ));
                }
            };

            println!("The server did the SOCKS thing and told me to go to {target_address}");
            target_address
        }
        TunnelTarget::Address(address) => {
            address.clone() // <-- TODO: This clone can definitely be avoided
        }
    };

    println!("Connecting connection from remote tunnel to {address}");
    let tcp_stream_result = bind_connect(address.as_ref()).await;

    let response_result = tcp_stream_result
        .as_ref()
        .map(|stream| stream.local_addr().unwrap_or(UNSPECIFIED_SOCKADDR_V4))
        .map_err(|error| {
            (OpenConnectionError::Connect, error) // TODO: Proper StartConnectionError value
        });

    match response_result {
        Ok(bind_address) => println!("Remote tunnel connected to {address} (local socket bound at {bind_address})"),
        Err((start_error, error)) => eprintln!("Remote tunnel failed to connect to target due to {start_error} failure: {error}"),
    }

    OpenRemoteConnectionResponseRef::new(response_result)
        .write(&mut send_stream)
        .await?;

    let mut tcp_stream = tcp_stream_result?;
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
