#![feature(extract_if)] // TODO: Remove once API is stabilized

use std::{
    env,
    future::{poll_fn, Future},
    io::Error,
    pin::Pin,
    process::exit,
    task::Poll,
};

use args::{ArgumentsRequest, StartupArguments};

use inlined::CompactVec;
use tokio::task::LocalSet;

use crate::{
    args::{ConnectMethod, StartupMode},
    endpoint::EndpointSocketSource,
    puncher::PunchConnectResult,
};

mod args;
mod client;
mod connect;
mod endpoint;
mod puncher;
mod server;
mod shared_socket;
mod socks;
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

    let (maybe_socket, mut addresses, mut background_task_handle) = match startup_args.connect_method {
        ConnectMethod::Direct(addresses) => (None, addresses, None),
        ConnectMethod::Punch(punch_config) => {
            let punch_result = connect::punch(punch_config, startup_args.startup_mode.is_server()).await?;

            let (socket, address, background_task_handle) = match punch_result {
                PunchConnectResult::Connect(socket, to_address) => {
                    let socket = EndpointSocketSource::Simple(socket.into_std()?);
                    (socket, to_address, None)
                }
                PunchConnectResult::Listen(socket, from_address, background_task_handle) => {
                    let socket = EndpointSocketSource::Shared(socket);
                    (socket, from_address, Some(background_task_handle))
                }
            };

            (Some(socket), CompactVec::from(address), background_task_handle)
        }
    };

    // On server mode, "addresses" is either the list of addresses to bind sockets at (with a non-holepunched connect
    // method, when `maybe_socket` is `None`), or the list of addresses to allow incoming connections from (when using
    // hole punching, when `maybe_socket` is `Some`).

    match startup_args.startup_mode {
        StartupMode::Client(client_config) => {
            let (endpoint, connection) = connect::connect_client(maybe_socket, addresses).await?;
            background_task_handle.inspect(|handle| handle.abort());

            match crate::client::run::run_client(connection, client_config).await {
                Ok(()) => {}
                Err(error) => eprintln!("Client finished with error: {error}"),
            }
            endpoint.wait_idle().await;
        }
        StartupMode::Server(_server_config) => {
            let (bind_addresses, address_filter) = match &maybe_socket {
                None => (addresses, None),
                Some(_) => (CompactVec::new(), Some(addresses.pop().unwrap())),
            };

            let endpoints = connect::connect_server(maybe_socket, bind_addresses).await?;

            let mut handles = Vec::new();
            handles.reserve_exact(endpoints.len());

            for endpoint in endpoints {
                let maybe_handle = background_task_handle.take();
                let handle = tokio::task::spawn_local(async move {
                    crate::server::run::run_server(endpoint, maybe_handle, address_filter).await;
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
