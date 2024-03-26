//! Types for inlining small collections for avoiding unnecessary heap allocations.
//!
//! It is common to need to use a vector to store a few small elements and end up using a [`Vec`].
//! This type allocates memory on the heap, and if you're only using a few elements then it is more
//! efficient to use the stack, or inline the vector into whatever structure you're operating on.
//!
//! For example, domain names are typically less than 100 characters and cannot be over 255.
//! Instead of using [`String`], that allocates memory on the heap both on creation and on clone,
//! you can use [`TinyString`], a 256-byte structure that can store up to 255 bytes using inline
//! memory, and cloning is as simple as copying memory:
//!
//! ```
//! use inlined::TinyString;
//! use std::fmt::Write;
//!
//! let mut s = TinyString::new();
//!
//! s.push_str("Hello!");
//! assert_eq!(s.as_str(), "Hello!");
//!
//! let _ = write!(s, " Your number is {}.", 1234);
//! assert_eq!(s.as_str(), "Hello! Your number is 1234.")
//! ```
//!
//! # Provided types
//!
//! This crate contains the following types:
//! - The [`InlineVec`] and [`InlineString`] types are analogous to [`Vec`] and [`String`] from the
//! standard library, but are inlined and have a constant, limited capacity.
//! - The [`TinyVec`] and [`TinyString`] types work much the same way, but use an `u8` for the
//! length instead of an `usize`. This makes them more optimal for passing around, or inlining them
//! into other structs.
//! - The [`CompactVec`] is a type that brings together [`Vec`] and [`TinyVec`], representing a
//! vector that stores up to `N` elements inline with, but if more capacity is needed will spill
//! into the heap and allocate memory.
//!
//! Since all of these implement [`Deref`](core::ops::Deref) for either `&[T]` or `&str`, they
//! contain many of the methods you're used to having from [`Vec`] and [`String`].
//!
//! # Specifying the capacity
//!
//! All these types make use of
//! [const generics](https://doc.rust-lang.org/reference/items/generics.html#const-generics) to
//! specify their inline capacity. This means you can choose the maximum amount of elements you
//! want your inlined type to store. Whether you want to store just 3, 100, or even thousands of
//! elements, the same types have got you covered.

#![feature(round_char_boundary)] // TODO: Remove once API is stabilized

#[cfg(test)]
mod test_utils;

pub mod compact_vec;
pub mod inline_string;
pub mod inline_vec;
pub mod tiny_string;
pub mod tiny_vec;

pub use compact_vec::CompactVec;
pub use inline_string::InlineString;
pub use inline_vec::InlineVec;
pub use tiny_string::TinyString;
pub use tiny_vec::TinyVec;
