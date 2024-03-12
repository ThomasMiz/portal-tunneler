use std::{
    future::{poll_fn, Future},
    io::{self, Error, ErrorKind, Write},
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6},
    num::NonZeroU16,
    pin::Pin,
    task::Poll,
};

use quinn::{Connection, Endpoint};
use tokio::{
    io::{stdin, AsyncBufReadExt, BufReader},
    task::JoinHandle,
};

use crate::{
    args::{ConnectMethod, PunchConfig, StartClientConfig, StartServerConfig},
    endpoint::{make_endpoint, EndpointSocketSource},
    puncher::{
        self,
        connection_code::{ConnectionCode, CONNECTION_STRING_MAX_LENGTH_CHARS},
        get_public_ip::get_public_ipv4,
        socket_binder::bind_sockets,
        PunchConnectResult,
    },
};

pub async fn punch(punch_config: PunchConfig, is_server: bool) -> io::Result<PunchConnectResult> {
    let port_start = punch_config.port_start.map(|p| p.get()).unwrap_or(0);
    let lane_count = punch_config.lane_count;

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
    let public_ip = match punch_config.my_ip {
        Some(ip) => match ip {
            IpAddr::V4(ipv4) => ipv4,
            IpAddr::V6(_) => panic!("Support for IPv6 is not implemented yet"),
        },
        None => get_public_ipv4().await?,
    };
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
    puncher::punch_connection(is_server, sockets, destination_code.address, remote_port_start, lane_count).await
}

pub async fn connect_client(_client_config: StartClientConfig, connect_method: ConnectMethod) -> io::Result<(Endpoint, Connection)> {
    match connect_method {
        ConnectMethod::Punch(punch_config) => {
            let (socket, destination_address) = match punch(punch_config, false).await? {
                PunchConnectResult::Listen(_, _, _) => panic!("Puncher state machine indicated to listen, but we're on client mode!"),
                PunchConnectResult::Connect(socket, destination_address) => (socket, destination_address),
            };

            let socket = socket.into_std()?;
            let endpoint = make_endpoint(EndpointSocketSource::Simple(socket), true, false)?;

            let connecting = endpoint
                .connect(destination_address, "localhost")
                .map_err(|e| Error::new(ErrorKind::Other, format!("Error while connecting to remote endpoint: {}", e)))?;

            let connection = connecting.await?;
            Ok((endpoint, connection))
        }
        ConnectMethod::Direct(addresses) => {
            let ipv4_endpoint = match addresses.iter().any(|a| a.is_ipv4()) {
                false => None,
                true => {
                    let bind_address = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0));
                    let result = std::net::UdpSocket::bind(bind_address)
                        .and_then(|socket| make_endpoint(EndpointSocketSource::Simple(socket), true, false));

                    match result {
                        Ok(endpoint) => Some(endpoint),
                        Err(error) => {
                            println!("Warning: Cannot use IPv4 because binding an IPv4 endpoint failed: {error}");
                            None
                        }
                    }
                }
            };

            let ipv6_endpoint = match addresses.iter().any(|a| a.is_ipv4()) {
                false => None,
                true => {
                    let bind_address = SocketAddr::V6(SocketAddrV6::new(Ipv6Addr::UNSPECIFIED, 0, 0, 0));
                    let result = std::net::UdpSocket::bind(bind_address)
                        .and_then(|socket| make_endpoint(EndpointSocketSource::Simple(socket), true, false));

                    match result {
                        Ok(endpoint) => Some(endpoint),
                        Err(error) => {
                            println!("Warning: Cannot use IPv6 because binding an IPv6 endpoint failed: {error}");
                            None
                        }
                    }
                }
            };

            let mut connect_futures = Vec::new();

            for address in addresses {
                let maybe_endpoint = match address {
                    SocketAddr::V4(_) => &ipv4_endpoint,
                    SocketAddr::V6(_) => &ipv6_endpoint,
                };

                let endpoint = match maybe_endpoint {
                    Some(endpoint) => endpoint,
                    None => continue,
                };

                match endpoint.connect(address, "localhost") {
                    Ok(c) => connect_futures.push(c),
                    Err(error) => println!("Couldn't start connection to {address}: {error}"),
                };
            }

            let result = poll_fn(move |cx| {
                let mut i = 0;
                while i < connect_futures.len() {
                    match Pin::new(&mut connect_futures[i]).poll(cx) {
                        Poll::Ready(Ok(connection)) => return Poll::Ready(Some(connection)),
                        Poll::Ready(Err(error)) => {
                            println!("Connection to {} failed: {error}", connect_futures[i].remote_address());
                            drop(connect_futures.swap_remove(i));
                        }
                        Poll::Pending => i += 1,
                    }
                }

                match connect_futures.is_empty() {
                    true => Poll::Ready(None),
                    false => Poll::Pending,
                }
            })
            .await;

            let connection = match result {
                Some(conn) => conn,
                None => {
                    return Err(Error::new(
                        ErrorKind::Other,
                        "Couldn't establish a connection to any of the provided addresses",
                    ))
                }
            };

            let endpoint = match connection.remote_address() {
                SocketAddr::V4(_) => ipv4_endpoint.unwrap(),
                SocketAddr::V6(_) => ipv6_endpoint.unwrap(),
            };

            Ok((endpoint, connection))
        }
    }
}

pub async fn connect_server(
    _server_config: StartServerConfig,
    connect_method: ConnectMethod,
) -> io::Result<(Vec<Endpoint>, Option<JoinHandle<()>>)> {
    match connect_method {
        ConnectMethod::Punch(punch_config) => {
            let (socket, background_task_handle) = match punch(punch_config, true).await? {
                PunchConnectResult::Listen(socket, _remote_address, handle) => (socket, handle),
                PunchConnectResult::Connect(_, _) => panic!("Puncher state machine indicated to connect, but we're on server mode!"),
            };

            let endpoint = make_endpoint(EndpointSocketSource::Shared(socket), false, true)?;
            Ok((vec![endpoint], Some(background_task_handle)))
        }
        ConnectMethod::Direct(addresses) => {
            let mut endpoints = Vec::new();

            for address in addresses {
                let socket = match std::net::UdpSocket::bind(address) {
                    Ok(so) => so,
                    Err(error) => {
                        println!("Couldn't bind socket at {address}: {error}");
                        continue;
                    }
                };

                match make_endpoint(EndpointSocketSource::Simple(socket), false, true) {
                    Ok(ep) => endpoints.push(ep),
                    Err(error) => {
                        println!("Couldn't create endpoint at {address}: {error}");
                        continue;
                    }
                }
            }

            match endpoints.is_empty() {
                true => Err(Error::new(
                    ErrorKind::Other,
                    "Couldn't bind a socket to any of the provided addresses",
                )),
                false => Ok((endpoints, None)),
            }
        }
    }
}
