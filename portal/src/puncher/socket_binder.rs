use std::{
    io::{self, Error, ErrorKind},
    net::SocketAddr,
    num::NonZeroU16,
};

/// Binds a set of `lane_count` sockets with sequentially increasing port numbers. The first socket
/// will be bound at the address `bind_address`, which may specify a port or may have port 0 to let
/// the OS choose a first port, and other sockets will be bound to the following ports.
///
/// If binding the first socket fails, whether it had a specific port or 0, the error will be
/// returned immediately. If binding one of the following sockets fails, then this function will
/// "fall back" to binding sockets in the ports _before_ the first one, rather than after (the
/// sockets will always be returned in ascending order by port).
///
/// If binding a non-first socket fails, even with the fallback, then if there was a specific port
/// an error will be returned immediately, otherwise the whole process will be retried up to three
/// times.
pub fn bind_sockets(bind_address: SocketAddr, lane_count: NonZeroU16) -> io::Result<Vec<tokio::net::UdpSocket>> {
    let mut sockets = Vec::with_capacity(lane_count.get() as usize);

    'outer: for _ in 0..3 {
        let first_socket = std::net::UdpSocket::bind(bind_address).and_then(|socket| {
            socket.set_nonblocking(true)?;
            tokio::net::UdpSocket::from_std(socket)
        })?;

        sockets.push(first_socket);
        let mut going_up = true;

        while sockets.len() < lane_count.get() as usize {
            let next_address = match going_up {
                true => {
                    let mut addr = sockets.last().unwrap().local_addr().unwrap();

                    let port = match addr.port() {
                        u16::MAX => {
                            going_up = false;
                            continue;
                        }
                        other => other + 1,
                    };

                    addr.set_port(port);
                    addr
                }
                false => {
                    let mut addr = sockets.first().unwrap().local_addr().unwrap();

                    let port = match addr.port() {
                        0 | 1 => {
                            sockets.clear();
                            continue 'outer;
                        }
                        other => other - 1,
                    };

                    addr.set_port(port);
                    addr
                }
            };

            let bind_result = std::net::UdpSocket::bind(next_address).and_then(|socket| {
                socket.set_nonblocking(true)?;
                tokio::net::UdpSocket::from_std(socket)
            });

            match bind_result {
                Ok(socket) if going_up => sockets.push(socket),
                Ok(socket) => sockets.insert(0, socket),
                Err(_) if bind_address.port() == 0 => {
                    sockets.clear();
                    continue 'outer;
                }
                Err(_) if going_up => {
                    going_up = false;
                    continue;
                }
                Err(_) => break 'outer,
            }
        }

        return Ok(sockets);
    }

    Err(Error::new(
        ErrorKind::AddrNotAvailable,
        "Couldn't bind the requested amount of sequential sockets.",
    ))
}
