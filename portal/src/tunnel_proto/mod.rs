//! This module describes the lightweight protocol built on top of QUIC for creating tunnels.
//!
//! The protocol makes use of bidirectional streams. It is important to note that all protocols
//! over QUIC streams are designed so the opener of the stream is the first one to talk, and will
//! always send at least one byte before closing the stream. This is because QUIC streams don't
//! exist for the other peer until the opener of the stream until actually sends bytes through it.
//! Opening and closing a stream without sending anything is invisible to the other side.
//!
//! Another thing to keep in mind is that all streams must be closed gracefully, using `finish()`.
//! This guarantees the other peer gets to see (and acknowledge) all the data from that stream.
//!
//! ## Terminology
//! Throughout this document, we will be using the terminology described in this subsection.
//!
//! ### Peers
//! This protocol operates on a client-server model, and thus one peer is denominated as the
//! _client_ and the other the _server_. These are respectively the same as the QUIC client and
//! server.
//!
//! ### Tunnels
//! A tunnel consists of a listening TCP socket on either the client or the server. When an
//! incoming connection is received on said socket, it is accepted and the other peer is told to
//! bind an active TCP socket and connect to the tunnel's _target_. Once that succeeds, both peers
//! start copying data bidirectionally.
//!
//! Tunnels can be categorized in two ways:
//! - By the location of the listening socket. A _local_ tunnel has the listening socket on the
//! client's side, while a _remote_ tunnel has it on the server's side.
//! - By the type of target. A tunnel's target may be a fixed address/domainname, or it may be a
//! SOCKS tunnel, so called a _dynamic_ tunnel, where each connection can specify the target to
//! connect to using the SOCKS protocol.
//!
//! Note that the listening socket for a tunnel can be specified as either an address or a
//! domainname. If the latter yields multiple addresses, then a tunnel may actually have multiple
//! listening TCP sockets.
//!
//! ### Tunnel lifecycle
//! It is important to distinguish a tunnel from the connections that go through it. A tunnel can
//! carry many connections, since each incoming TCP connection received by the listening socket
//! will create another connection that passes through the tunnel.
//!
//! For this, we distinguish the lifecycles between the _tunnel_ and the _tunnel connections_
//! carried through it. A tunnel is _started_ and _ended_, and during this time multiple tunnel
//! connections may be _opened_ and _closed_. This small differentiation in terminology is used to
//! disambiguate when talking about the lifecycle of tunnels and connections.
//!
//! # Client-opened bidirectional streams
//! When the client opens a bidirectional stream, it can be for either opening a new connection on
//! a local tunnel or for requesting the server starts new remote tunnels (which, as connections
//! come in through it, the server will open bidirectional streams as indicated in the following
//! section). This means that the server is never told all the local tunnels, and remote tunnels
//! may be opened at any time.
//!
//! In both cases, the client opens the stream and is the first to talk. The way to distinguish
//! between what the stream is for is with the first byte sent. If this byte is 0, this stream is
//! for a new connection on a local tunnel. If the byte is 1, the stream is for requesting the
//! server starts a remote tunnel (see: [`ClientStreamRequest`](requests::ClientStreamRequest)).
//!
//! ## Streams for a local tunnel
//! The client first sends an [`AddressOrDomainname`](types::AddressOrDomainname) indicating the
//! destination (the reason why a domainname can be specified is because we want the server to do
//! the DNS query). The server then attempts the connection and responds with either the address of
//! the newly bound socket, or the error that occurred:
//! `Result<SocketAddr, (OpenConnectionError, Error)>`. If the server responds with an error, it
//! must then close the stream. Otherwise, the connection has been established and both the server
//! an the client can start copying user data bidirectionally. If either side sees the connection
//! closed, it must close the stream. Note that if the tunnel's target is dynamic (e.g. SOCKS) then
//! the server does not see that, as that's handled by the client.
//!
//! ## Streams for requesting starting a remote tunnel
//! The client first sends the ID of the tunnel, an `u32`, followed by the target type of the
//! tunnel (whether it's static or SOCKS), an
//! [`TunnelTargetType`](remote_tunnels::TunnelTargetType), and finally the address of the socket
//! to listen for incoming connections at, an [`AddressOrDomainname`](types::AddressOrDomainname).
//! All these are encompassed by the type
//! [`StartRemoteTunnelRequest`](remote_tunnels::StartRemoteTunnelRequest). The server then
//! responds with either an error (if the socket couldn't be bound) or an Ok():
//! `Result<(), Error>`. The client may send one or multiple of these requests in a pipelined
//! fashion, and then must close its sending end of the stream. The server must finish answering
//! all requests, responding them in the same order as sent, and then close the stream.
//!
//! # Server-opened bidirectional streams
//! Server-opened streams are exclusively used for new connections in a remote tunnel. The server
//! starts by sending an `u32`, the ID of the tunnel, followed by the optional target, an
//! `Option<AddressOrDomainname>`, which should be `None` if the tunnel target type is static or
//! `Some` otherwise (the server knows the target type, as that was specified by the client when
//! it requested opening the tunnel). The client then attempts the connection and responds with
//! either the address of the newly bound socket, or the error that occurred:
//! `Result<SocketAddr, (OpenConnectionError, Error)>`. If the client responds with an error, it
//! must then close the stream. Otherwise, the connection has been established and both the server
//! and the client can start copying user data bidirectionally. If either side sees the connection
//! closed, it must close the stream. Note that if the tunnel's target is dynamic (e.g. SOCKS) then
//! the dynamic protocol is in this case handled by the server.

// TODO: Protocol version cannot be negotiated during hole punching, because we might not be doing
// hole punching (oops). Change this.
/// The version of the protocol. This is negotiated during hole punching through the application
/// data. Each peer will see the other's protocol version, so the minimum between the two will be
/// used.
///
/// Note: This is currently the only version of the protocol.
pub const PROTOCOL_VERSION: u16 = 1;

pub mod local_tunnels;
pub mod remote_tunnels;
pub mod requests;
pub mod responses;
pub mod serialize;
pub mod types;
pub mod u8_repr_enum;
