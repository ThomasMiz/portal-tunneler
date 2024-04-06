use std::{io, rc::Rc};

use portal_tunneler_proto::{
    client::ClientState,
    serialize::{ByteRead, ByteWrite},
    shared::{ClientStreamRequest, OpenLocalConnectionRequestRef, OpenLocalConnectionResponse, TunnelSpec, TunnelTarget},
};

use tokio::{
    net::{TcpListener, TcpStream},
    try_join,
};

use crate::socks;

pub async fn handle_local_tunnel_listening(client: Rc<ClientState>, listener: TcpListener, spec: Rc<TunnelSpec>) {
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

        let client = Rc::clone(&client);
        let spec = Rc::clone(&spec);
        tokio::task::spawn_local(async move {
            match handle_local_tunnel(client, tcp_stream, spec).await {
                Ok(()) => {}
                Err(error) => println!("Local tunnel task finished with error: {error}"),
            }
        });
    }
}

pub async fn handle_local_tunnel(client: Rc<ClientState>, mut tcp_stream: TcpStream, spec: Rc<TunnelSpec>) -> io::Result<()> {
    let (mut read_half, mut write_half) = tcp_stream.split();

    let maybe_socks_target;
    let (maybe_socks_version, target) = match &spec.target {
        TunnelTarget::Socks => {
            let request_result = socks::read_request(&mut read_half, &mut write_half).await;

            if let Err(socks_error) = &request_result {
                println!("Socks error: {socks_error}");
                socks::send_request_error(&mut write_half, socks_error).await?;
            }

            let (version, target) = request_result?;
            maybe_socks_target = Some(target);
            (Some(version), maybe_socks_target.as_ref().unwrap().as_ref())
        }
        TunnelTarget::Address(address) => (None, address.as_ref()),
    };

    let (mut send_stream, mut recv_stream) = client.connection().open_bi().await?;
    ClientStreamRequest::OpenLocalTunnelConnection.write(&mut send_stream).await?;

    let request = OpenLocalConnectionRequestRef::new(target);
    request.write(&mut send_stream).await?;

    let response = OpenLocalConnectionResponse::read(&mut recv_stream).await?;
    if let Err((start_error, error)) = &response.result {
        eprintln!("Failed to connect local tunnel, server responded with {start_error} failure: {error}");
    }

    if let Some(socks_version) = maybe_socks_version {
        socks::send_response(&mut write_half, socks_version, &response.result).await?;
    }

    let bind_address = response.result.map_err(|(_, error)| error)?;

    println!("Local tunnel connected through server (remote socket bound at {bind_address})");

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
