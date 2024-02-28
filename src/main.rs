use std::{
    env,
    io::{Error, ErrorKind, Write},
    net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4},
    process::exit,
};

use args::{ArgumentsRequest, StartupArguments};
use puncher::PuncherStarter;
use tokio::{
    io::{stdin, AsyncBufReadExt, AsyncReadExt, BufReader},
    task::LocalSet,
};

use crate::puncher::{
    connection_code::{self, ConnectionCode, CONNECTION_STRING_MAX_LENGTH_CHARS},
    get_public_ip::get_public_ipv4,
};

mod args;
mod client;
mod puncher;
mod server;
mod shared_socket;
mod utils;

pub const PORT: u16 = 4949;

pub const KEEPALIVE_INTERVAL_PERIOD_MILLIS: u64 = 3000;
pub const MAX_IDLE_TIMEOUT_MILLIS: u32 = 5000;

fn main() {
    let arguments = match args::parse_arguments(env::args()) {
        Err(err) => {
            eprintln!("{err}\n\nType 'dust-devil --help' for a help menu");
            exit(1);
        }
        Ok(arguments) => arguments,
    };

    let startup_args = match arguments {
        ArgumentsRequest::Version => {
            println!("{}", args::get_version_string());
            println!("Your mother's favorite socks5 proxy server");
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
    print!("Binding sockets...");
    let bind_address = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0));
    let starter = PuncherStarter::new(bind_address, startup_args.lane_count)?;
    println!(" Done");

    print!("Finding your public IP address...");
    let public_ip = get_public_ipv4().await?;
    println!(" {public_ip}");

    let connection_code = starter.generate_connection_code(IpAddr::V4(public_ip));
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

    let puncher = starter.set_remote(destination_code.address, destination_code.port_start, destination_code.lane_count)?;
    println!("Puncher: {puncher:?}");

    /*if startup_args.is_server {
        server::run_server().await;
    } else {
        client::run_client().await;
    }*/

    Ok(())
}
