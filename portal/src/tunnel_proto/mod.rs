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
//! between what the stream is for is with the highest bit of the first byte sent. If this bit is
//! 0, this stream is for a local tunnel. If the bit is 1, the stream is for requesting the server
//! starts a remote tunnel. This bit should then be set to 0 and proceed with one of the two
//! procedures describe in the following sub-sections.
//!
//! ## Streams for a local tunnel
//! The client first sends an [`AddressOrDomainname`](types::AddressOrDomainname) indicating the
//! destination (the reason why a domainname can be specified is because we want the server to do
//! the DNS query). The server then responds with either the address of the newly bound socket, or
//! the error that occurred: `Result<SocketAddr, (StartConnectionError, Error)>`. If the server
//! responds with an error, it must then close the stream. Otherwise, the connection has been
//! established and both the server an the client can start copying user data bidirectionally. If
//! either side sees the connection closed, it must close the stream.
//!
//! ## Streams for requesting starting a remote tunnel
//! The client first sends an [`AddressOrDomainname`](types::AddressOrDomainname) indicating the
//! address of the socket to listen for incoming connections at, followed by an `u32`, the ID of
//! the tunnel. The server then responds with either an error (if the socket couldn't be bound) or
//! an Ok(): `Result<(), Error>`. The client may send one or multiple of these requests in a
//! pipelined fashion, and then must close its sending end of the stream. The server must finish
//! answering all requests, responding them in the same order as sent, and then close the stream.
//!
//! # Server-opened bidirectional streams
//! Server-opened streams are exclusively used for new connections in a remote tunnel. The server
//! starts by sending an `u32`, the ID of the tunnel. The client then responds with a single `bool`
//! indicating whether the connection is established. If false, the client must then close the
//! stream. Otherwise, the connection has been established and the server can start copying user
//! data bidirectionally. The server does not know where the tunnel goes, or even if it is a
//! dynamic SOCKS tunnel. All of that is handled by the client.

/// The version of the protocol. This is negotiated during hole punching through the application
/// data. Each peer will see the other's protocol version, so the minimum between the two will be
/// used.
///
/// Note: This is currently the only version of the protocol.
pub const PROTOCOL_VERSION: u16 = 1;

pub mod types;

mod serialize;
mod u8_repr_enum;
