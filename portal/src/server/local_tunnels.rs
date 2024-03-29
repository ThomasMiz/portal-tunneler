use std::io;

use quinn::{RecvStream, SendStream};
use tokio::try_join;

use crate::{
    tunnel_proto::{
        local_tunnels::{OpenLocalConnectionRequest, OpenLocalConnectionResponseRef},
        responses::OpenConnectionError,
        serialize::{ByteRead, ByteWrite},
    },
    utils::{bind_connect, UNSPECIFIED_SOCKADDR_V4},
};

pub async fn handle_open_local_tunnel_stream(mut send_stream: SendStream, mut recv_stream: RecvStream) -> io::Result<()> {
    println!("Incoming connection from on tunnel");

    let request = OpenLocalConnectionRequest::read(&mut recv_stream).await?;
    println!("Connecting connection from remote tunnel to {}", request.target);

    let tcp_stream_result = bind_connect(request.target.as_ref()).await;

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
