use quinn::{Connecting, Endpoint, RecvStream, SendStream, VarInt};
use tokio::{
    io::{stdin, stdout},
    join, select,
    task::{AbortHandle, JoinHandle},
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

        println!("Incoming connection form addr={}", incoming_connection.remote_address());
        let hhh = abort_on_connect.as_ref().map(|h| h.abort_handle());
        tokio::task::spawn_local(async move {
            handle_connection(incoming_connection, hhh).await;
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

    loop {
        let (send_stream, recv_stream) = match connection.accept_bi().await {
            Ok(v) => v,
            Err(error) => {
                println!("Failed to accept bidirectional stream: {error}");
                break;
            }
        };

        println!("Accepted bidirectional stream {} {}", send_stream.id(), recv_stream.id());
        tokio::task::spawn_local(async move {
            handle_bi_stream(send_stream, recv_stream).await;
        });
    }
}

async fn handle_bi_stream(mut send_stream: SendStream, mut recv_stream: RecvStream) {
    println!("Doing bidirectional copy");

    let mut stdout = stdout();
    let mut stdin = stdin();
    let (r1, r2) = join!(
        tokio::io::copy(&mut recv_stream, &mut stdout),
        tokio::io::copy(&mut stdin, &mut send_stream),
    );

    println!("Finished stream:\nstream-to-stdout result: {r1:?}\nstdin-to-stream result: {r2:?}");
}
