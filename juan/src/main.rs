use std::{
    env,
    io::{Error, ErrorKind, Write},
    net::{IpAddr, Ipv4Addr, SocketAddr},
    num::NonZeroU16,
    process::exit,
};

use args::{ArgumentsRequest, StartupArguments};
use tokio::{
    io::{stdin, AsyncBufReadExt, BufReader},
    task::LocalSet,
};

use crate::puncher::{
    connection_code::{ConnectionCode, CONNECTION_STRING_MAX_LENGTH_CHARS},
    get_public_ip::get_public_ipv4,
    socket_binder::bind_sockets,
};

mod args;
mod client;
mod puncher;
mod server;
mod shared_socket;
mod utils;

pub const PORT: u16 = 4949;

pub const KEEPALIVE_INTERVAL_PERIOD_MILLIS: u64 = 5000;
pub const MAX_IDLE_TIMEOUT_MILLIS: u32 = 24000;

fn main() {
    let arguments = match args::parse_arguments(env::args()) {
        Err(err) => {
            eprintln!("{err}\n\nType 'juan --help' for a help menu");
            exit(1);
        }
        Ok(arguments) => arguments,
    };

    let startup_args = match arguments {
        ArgumentsRequest::Version => {
            println!("{}", args::get_version_string());
            println!("Do NOT ask who Juan is.");
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
    let port_start = startup_args.port_start.map(|p| p.get()).unwrap_or(0);
    let lane_count = startup_args.lane_count;

    print!("Binding sockets...");
    std::io::stdout().flush()?;
    let sockets = bind_sockets(SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), port_start), lane_count)?;
    let port_start = sockets[0].local_addr().unwrap().port();

    if sockets.len() == 1 {
        println!(" Done, bound a single socket at {}", sockets.first().unwrap().local_addr().unwrap());
    } else {
        let first_addr = sockets.first().unwrap().local_addr().unwrap();
        let last_addr = sockets.last().unwrap().local_addr().unwrap();
        println!(" Done, bound {} sockets from {} to {}", sockets.len(), first_addr, last_addr);
    }

    print!("Finding your public IP address...");
    std::io::stdout().flush()?;
    let public_ip = get_public_ipv4().await?;
    println!(" {public_ip}");

    let connection_code = ConnectionCode::new(IpAddr::V4(public_ip), port_start, lane_count);
    println!("Your connection code is: {}", connection_code.serialize_to_string());

    print!("Enter your friend's connection code: ");
    std::io::stdout().flush()?;
    let mut s = String::with_capacity(CONNECTION_STRING_MAX_LENGTH_CHARS + 2);
    let mut stdin = BufReader::with_capacity(1024, stdin());
    stdin.read_line(&mut s).await?;
    let destination_code = ConnectionCode::deserialize_from_str(s.trim()).map_err(|e| {
        let message = format!("Invalid error code: {e:?}");
        Error::new(ErrorKind::InvalidData, message)
    })?;

    if connection_code.lane_count != destination_code.lane_count {
        println!("Warning! The lane counts on the connection codes don't match. The minimum will be used.");
        println!(
            "Local lane count: {}, Remote lane count: {}",
            connection_code.lane_count, destination_code.lane_count
        );
    }

    let remote_port_start = NonZeroU16::new(destination_code.port_start).unwrap();
    let lane_count = connection_code.lane_count.min(destination_code.lane_count);

    println!("Punching!");
    let (socket, maybe_background_task, remote_address) = puncher::punch_connection(
        startup_args.is_server,
        sockets,
        destination_code.address,
        remote_port_start,
        lane_count,
    )
    .await?;

    if startup_args.is_server {
        server::run_server(socket, maybe_background_task.map(|h| h.abort_handle())).await;
    } else {
        client::run_client(socket, remote_address).await;
    }

    Ok(())
}
