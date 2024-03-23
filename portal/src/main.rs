#![feature(round_char_boundary)] // TODO: Remove once API is stabilized

use std::{
    env,
    future::{poll_fn, Future},
    io::Error,
    pin::Pin,
    process::exit,
    task::Poll,
};

use args::{ArgumentsRequest, StartupArguments};

use tokio::task::LocalSet;

use crate::{
    args::StartupMode,
    connect::{connect_client, connect_server},
};

mod args;
mod client;
mod connect;
mod endpoint;
mod puncher;
mod server;
mod shared_socket;
mod tunnel_proto;
mod utils;

fn main() {
    let arguments = match args::parse_arguments(env::args()) {
        Err(err) => {
            eprintln!("{err}\n\nType 'portal --help' for a help menu");
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

    match startup_args.startup_mode {
        StartupMode::Client(client_config) => {
            let (endpoint, connection) = connect_client(startup_args.connect_method).await?;
            match client::run_client(connection, client_config.tunnels).await {
                Ok(()) => {}
                Err(error) => eprintln!("Client finished with error: {error}"),
            }
            endpoint.wait_idle().await;
        }
        StartupMode::Server(_server_config) => {
            let (endpoints, mut maybe_handle) = connect_server(startup_args.connect_method).await?;
            let mut handles = Vec::with_capacity(endpoints.len());
            for endpoint in endpoints {
                let maybe_handle = maybe_handle.take();
                let handle = tokio::task::spawn_local(async move {
                    server::run_server(endpoint, maybe_handle).await;
                });

                handles.push(handle);
            }

            poll_fn(move |cx| {
                let mut i = 0;
                while i < handles.len() {
                    match Pin::new(&mut handles[i]).poll(cx) {
                        Poll::Ready(_) => {
                            handles.swap_remove(i);
                        }
                        Poll::Pending => i += 1,
                    }
                }

                match handles.is_empty() {
                    true => Poll::Ready(()),
                    false => Poll::Pending,
                }
            })
            .await;
        }
    }

    Ok(())
}
