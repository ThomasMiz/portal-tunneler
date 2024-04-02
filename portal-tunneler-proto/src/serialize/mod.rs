//! Defines the [`ByteRead`] and [`ByteWrite`] traits and implements them for many basic types.
//!
//! This includes `()`, [`bool`], [`u8`], [`u16`], [`u32`], [`u64`], [`i64`] and [`char`], as well
//! as more complex types, including [`str`] (write-only), [`String`], `[T]` (write-only),
//! [`Vec<T>`], [`Ipv4Addr`](std::net::Ipv4Addr), [`Ipv6Addr`](std::net::Ipv6Addr),
//! [`SocketAddrV4`](std::net::SocketAddrV4), [`SocketAddrV6`](std::net::SocketAddrV6),
//! [`SocketAddr`](std::net::SocketAddr), [`Option<T>`], [`Result<T, E>`] and
//! [`Error`](std::io::Error).
//!
//! # Serialization of [`Option<T>`] and [`Result<T, E>`]
//! [`Option<T>`] types have [`ByteRead`] and [`ByteWrite`] implemented for `T: ByteRead`
//! and/or `T: ByteWrite` respectively. Serializing this consists of a presence byte, 1 if Some and
//! 0 if None, and if 1 then this byte is followed by the serialization of `T`.
//!
//! A similar strategy is used for [`Result<T, E>`], with the exception that if the presence byte
//! is 0 then it is followed by the serialization of `E`.
//!
//! # Serialization of [`Error`](std::io::Error)
//! [`Error`](std::io::Error) is serialized with the kind and it's `.to_string()`, as to preserve
//! as much information on the error as possible.
//!
//! # Serialization of strings and lists
//! [`String`] and [`str`] are serialized as chunked strings, starting with an [`u16`] indicating
//! the length of the string in bytes, followed by said amount of bytes. Some strings however are
//! not allowed to be longer than 255 bytes, particularly domain names, usernames and passwords, so
//! these are serialized with [`u8`] length instead through the [`SmallReadString`] and
//! [`SmallWriteString`] types, which wrap a [`String`] and an `&str` respectively.
//!
//! [`Vec<T>`] and slices are also serialized as chunked lists, starting with an [`u16`] indicating
//! the length, followed by said amount of elements. Just like with strings, the [`SmallReadList`]
//! and [`SmallWriteList`] types are provided, which wrap a [`Vec<T>`] and `&[T]` respectively.
//!
//! # Serialization of tuples
//! [`ByteRead`] and [`ByteWrite`] are also implemented for any tuple of up to 5 elements, with all
//! the element types being [`ByteRead`] and/or [`ByteWrite`]. This allows easily turning multiple
//! writes such as this:
//! ```ignore
//! thing1.write(writer).await?;
//! thing2.write(writer).await?;
//! thing3.write(writer).await?;
//! thing4.write(writer).await?;
//! ```
//!
//! into this:
//! ```ignore
//! (thing1, thing2, thing3, thing4).write(writer).await?;
//! ```

use std::io;

use tokio::io::{AsyncRead, AsyncWrite};

pub mod enums;
pub mod error;
pub mod lists;
pub mod net;
pub mod nonzero;
pub mod primitives;
pub mod string;
pub mod tuples;
pub mod u8_repr_enum;

pub use lists::*;
pub use string::*;
pub use u8_repr_enum::*;

/// Serializes a type into bytes, writing it to an [`AsyncWrite`] asynchronously.
#[allow(async_fn_in_trait)]
pub trait ByteWrite {
    /// Serializes this instance into bytes, writing those bytes into a writer.
    ///
    /// When an error occurs, there's no guarantee on how many bytes were written.
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> io::Result<()>;
}

/// Deserializes a type from raw bytes, reading it from an [`AsyncRead`] asynchronously.
#[allow(async_fn_in_trait)]
pub trait ByteRead: Sized {
    /// Deserializes bytes into an instance of this type by reading bytes from a reader.
    ///
    /// When an error occurs, there's no guarantee on how many bytes were read.
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> io::Result<Self>;
}
