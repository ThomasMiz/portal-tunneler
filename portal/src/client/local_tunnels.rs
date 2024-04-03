use std::{io, rc::Rc};

use portal_tunneler_proto::{
    serialize::{ByteRead, ByteWrite},
    shared::{ClientStreamRequest, OpenLocalConnectionRequestRef, OpenLocalConnectionResponse, TunnelSpec, TunnelTarget},
};
use quinn::Connection;
use tokio::{
    net::{TcpListener, TcpStream},
    try_join,
};

pub async fn handle_local_tunnel_listening(connection: Rc<Connection>, listener: TcpListener, spec: Rc<TunnelSpec>) {
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

pub async fn handle_local_tunnel(connection: Rc<Connection>, mut tcp_stream: TcpStream, spec: Rc<TunnelSpec>) -> io::Result<()> {
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
