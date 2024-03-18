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
//! # Client-opened bidirectional streams
//! When the client opens a bidirectional stream, it can be for either opening a local tunnel
//! (which comes out of the server), or for requesting the server starts a remote tunnel (which,
//! as connections come in through it, the server will open bidirectional streams as indicated in
//! the following section). This means that the server is never told all the local tunnels, and
//! remote tunnels may be opened at any time.
//!
//! In both cases, the client opens the stream and is the first to talk. The way to distinguish
//! between what the stream is for is with the first byte sent. If this byte is 0, this stream is
//! for a new connection on a local tunnel. If the byte is 1, the stream is for requesting the
//! server starts a remote tunnel (see: [`ClientStreamRequest`](types::ClientStreamRequest)).
//!
//! ## Streams for a local tunnel
//! The client first sends an [`AddressOrDomainname`](types::AddressOrDomainname) indicating the
//! destination (the reason why a domainname can be specified is because we want the server to do
//! the DNS query). The server then attempts the connection and responds with either the address of
//! the newly bound socket, or the error that occurred:
//! `Result<SocketAddr, (StartConnectionError, Error)>`. If the server responds with an error, it
//! must then close the stream. Otherwise, the connection has been established and both the server
//! an the client can start copying user data bidirectionally. If either side sees the connection
//! closed, it must close the stream. Note that if the tunnel's target is dynamic (e.g. SOCKS) then
//! the server does not see that, as that's handled by the client.
//!
//! ## Streams for requesting starting a remote tunnel
//! The client first sends the ID of the tunnel, an `u32`, followed by the target type of the
//! tunnel (whether it's static or SOCKS), an [`TunnelTargetType`](types::TunnelTargetType), and
//! finally the address of the socket to listen for incoming connections at, an
//! [`AddressOrDomainname`](types::AddressOrDomainname). All these are encompassed by the type
//! [`OpenRemoteTunnelRequest`](types::OpenRemoteTunnelRequest). The server then responds with
//! either an error (if the socket couldn't be bound) or an Ok(): `Result<(), Error>`. The client
//! may send one or multiple of these requests in a pipelined fashion, and then must close its
//! sending end of the stream. The server must finish answering all requests, responding them in
//! the same order as sent, and then close the stream.
//!
//! # Server-opened bidirectional streams
//! Server-opened streams are exclusively used for new connections in a remote tunnel. The server
//! starts by sending an `u32`, the ID of the tunnel, followed by the optional target, an
//! `Option<AddressOrDomainname>`, which should be `None` if the tunnel target type is static or
//! `Some` otherwise (the server knows the target type, as that was specified by the client when
//! it requested opening the tunnel). The client then attempts the connection and responds with
//! either the address of the newly bound socket, or the error that occurred:
//! `Result<SocketAddr, (StartConnectionError, Error)>`. If the client responds with an error, it
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

pub mod types;

pub mod serialize;
pub mod u8_repr_enum;
