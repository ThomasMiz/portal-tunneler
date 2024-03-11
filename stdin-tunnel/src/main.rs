use std::{env, io::Error, process::exit};

use args::{ArgumentsRequest, StartupArguments};
use tokio::{
    io::{stdin, AsyncReadExt},
    select,
    task::LocalSet,
};

mod args;
mod puncher;
mod shared_socket;
mod utils;

fn main() {
    let arguments = match args::parse_arguments(env::args()) {
        Err(err) => {
            eprintln!("{err}\n\nType 'stdin-tunnel --help' for a help menu");
            exit(1);
        }
        Ok(arguments) => arguments,
    };

    let startup_args = match arguments {
        ArgumentsRequest::Version => {
            println!("{}", args::get_version_string());
            println!("// TODO: Write a funny message");
            return;
        }
        ArgumentsRequest::Help => {
            println!("{}", args::get_help_string());
            return;
        }
        ArgumentsRequest::Run(startup_args) => startup_args,
    };

    let runtime_result = tokio::runtime::Builder::new_current_thread().enable_all().build();

    let result = match runtime_result {
        Ok(runtime) => LocalSet::new().block_on(&runtime, async_main(startup_args)),
        Err(err) => {
            eprintln!("Failed to start Tokio runtime: {err}");
            exit(1);
        }
    };

    if let Err(error) = result {
        println!("Program finished with error: {error}\n\nDebug print: {error:?}");
    }
}

async fn async_main(startup_args: StartupArguments) -> Result<(), Error> {
    println!("Startup arguments: {startup_args:?}");

    println!("Punching...");
    let (socket, mut maybe_handle, remote_address) = puncher::punch(
        startup_args.is_server,
        startup_args.port_start,
        startup_args.remote_address,
        startup_args.remote_port_start,
        startup_args.lane_count,
    )
    .await?;

    println!("==================================================");

    if !startup_args.is_server {
        socket.send_to(b"Buenos Dias, Fuckboi", remote_address).await.unwrap();
        println!("First message sent!");
    }

    let mut stdin = stdin();
    let mut read_buf = [0u8; 1400];
    let mut recv_buf = [0u8; 1400];

    loop {
        select! {
            result = stdin.read(&mut read_buf) => {
                match result {
                    Ok(len) => {
                        match socket.send_to(&read_buf[..len], remote_address).await {
                            Ok(sent) => {
                                read_buf[..len].iter_mut().filter(|b| **b != b' ' && !b.is_ascii_graphic()).for_each(|b| *b = b'?');
                                println!("Received {len} bytes, sent {sent} from {} to {}: {}", socket.local_addr().unwrap(), remote_address, std::str::from_utf8(&read_buf[..len]).unwrap());
                            },
                            Err(error) => println!("Socket {} send_to to {remote_address} error: {error}", socket.local_addr().unwrap()),
                        }
                    },
                    Err(error) => println!("Stdin read error: {error}"),
                }
            }
            result = socket.recv_from(&mut recv_buf) => {
                match result {
                    Ok((len, from)) => {
                        if from == remote_address {
                            maybe_handle.take().inspect(|h| h.abort());
                        }
                        recv_buf[..len].iter_mut().filter(|b| **b != b' ' && !b.is_ascii_graphic()).for_each(|b| *b = b'?');
                        println!("Socket {} received {len} bytes from {from}: {}", socket.local_addr().unwrap(), std::str::from_utf8(&recv_buf[..len]).unwrap());
                    },
                    Err(error) => println!("Socket {} recv_from error: {error}", socket.local_addr().unwrap()),
                }
            }
        }
    }
}
