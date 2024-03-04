use std::{env, io::Error, process::exit};

use args::{ArgumentsRequest, StartupArguments};
use tokio::task::LocalSet;

use crate::{client::run_client, server::run_server};

mod args;
mod client;
mod puncher;
mod server;
mod shared_socket;
mod utils;

pub const KEEPALIVE_INTERVAL_PERIOD_MILLIS: u64 = 6000;
pub const MAX_IDLE_TIMEOUT_MILLIS: u32 = 20000;

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
    let (socket, maybe_handle, remote_address) = puncher::punch(
        startup_args.is_server,
        startup_args.port_start,
        startup_args.remote_address,
        startup_args.remote_port_start,
        startup_args.lane_count,
    )
    .await?;

    println!("==================================================");

    if startup_args.is_server {
        println!("Starting up server");
        run_server(socket, maybe_handle.map(|handle| handle.abort_handle())).await;
    } else {
        println!("Starting up client");
        run_client(socket, remote_address).await;
    }

    println!("Done");
    Ok(())
}
