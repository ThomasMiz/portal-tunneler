use std::net::SocketAddr;

use quinn::{Endpoint, RecvStream, SendStream, VarInt};
use tokio::{
    io::{stdin, stdout},
    join,
};

pub async fn run_client(endpoint: Endpoint, server_addr: SocketAddr) {
    let local_addr = endpoint.local_addr().unwrap();

    let connection = loop {
        println!("Attempting to connect from {local_addr} to {server_addr}");
        let connecting = match endpoint.connect(server_addr, "localhost") {
            Ok(c) => c,
            Err(e) => {
                println!("Connect error: {e}");
                continue;
            }
        };

        println!("Connecting!");
        match connecting.await {
            Ok(c) => break c,
            Err(e) => println!("Connecting error: {e}"),
        }
    };

    println!("Client connected to {server_addr}");

    // IMPORTANT NOTE: QUIC streams are not received on the other end until actually used!
    let (send_stream, recv_stream) = match connection.open_bi().await {
        Ok(t) => t,
        Err(error) => {
            println!("Failed to open bidirectional stream: {error}");
            return;
        }
    };

    println!("Opened bidirectional stream {} {}", send_stream.id(), recv_stream.id());
    handle_bi_stream(send_stream, recv_stream).await;

    endpoint.close(VarInt::default(), b"Adios, fuckbois!");
    endpoint.wait_idle().await;
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
