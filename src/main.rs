use std::{env, io::Error, process::exit};

use args::{ArgumentsRequest, StartupArguments};

mod args;
mod client;
mod server;
mod utils;

pub const PORT: u16 = 4949;

pub const MAX_DATAGRAM_SIZE: usize = 1350;
pub const MAX_IDLE_TIMEOUT: u64 = 5000;

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

    let runtime_result = tokio::runtime::Builder::new_multi_thread().enable_all().build();

    let result = match runtime_result {
        Ok(runtime) => runtime.block_on(async_main(startup_args)),
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
    if startup_args.is_server {
        server::run_server().await
    } else {
        client::run_client().await
    }
}
