use std::{
    io::{self, Error, ErrorKind},
    rc::Rc,
};

use portal_tunneler_proto::{
    client::ClientState,
    serialize::{ByteRead, ByteWrite},
    shared::{OpenConnectionError, OpenRemoteConnectionRequest, OpenRemoteConnectionResponseRef, TunnelTarget},
};
use quinn::{RecvStream, SendStream};
use tokio::try_join;

use crate::utils::{bind_connect, UNSPECIFIED_SOCKADDR_V4};

pub async fn handle_incoming_bi_stream(
    client: Rc<ClientState>,
    mut send_stream: SendStream,
    mut recv_stream: RecvStream,
) -> io::Result<()> {
    // Incoming (server-opened) bidi streams are exclusively used for new connections in a remote tunnel.

    println!("Incoming connection from remote tunnel");

    let request = OpenRemoteConnectionRequest::read(&mut recv_stream).await?;

    let spec = match client.lock().get_remote_tunnel(request.tunnel_id) {
        Some(spec) => spec,
        None => {
            eprintln!("Error: Server opened a new tunnel but specified invalid tunnel ID");
            return Err(Error::new(ErrorKind::Other, "Server specified invalid tunnel ID"));
        }
    };

    let maybe_target_address;
    let address = match &spec.target {
        TunnelTarget::Socks => {
            maybe_target_address = match request.maybe_target {
                Some(addr) => addr,
                None => {
                    eprintln!("The server specified an address as target on a static tunnel");
                    return Err(Error::new(
                        ErrorKind::Other,
                        "The server specified an address as target on a static tunnel",
                    ));
                }
            };

            println!("The server did the SOCKS thing and told me to go to {maybe_target_address}");
            &maybe_target_address
        }
        TunnelTarget::Address(address) => address,
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
