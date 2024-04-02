//! Provides the [`U8ReprEnum`] trait, which is made to be implemented by enums that can be
//! converted into or parsed from an [`u8`] value, for easy serialization and deserialization.

/*use std::{
    any::type_name,
    io::{self, Error, ErrorKind},
};

use super::{ByteRead, ByteWrite};

use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};*/

/// Allows a type to be converted into or parsed from an [`u8`] representation.
pub trait U8ReprEnum: Sized + Copy {
    /// Parses an `u8` into the enum variant it represents. If the `u8` represents a variant in
    /// this enum, then `Some` is returned with said variant. Otherwise, `None` is returned.
    fn from_u8(value: u8) -> Option<Self>;

    /// Converts this enum into its `u8` representation.
    fn into_u8(self) -> u8;
}

/*impl<T: U8ReprEnum> ByteWrite for T {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> io::Result<()> {
        self.into_u8().write(writer).await
    }
}

impl<T: U8ReprEnum> ByteRead for T {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> io::Result<Self> {
        match Self::from_u8(u8::read(reader).await?) {
            Some(role) => Ok(role),
            None => Err(Error::new(
                ErrorKind::InvalidData,
                format!("Invalid {} type byte", type_name::<T>()),
            )),
        }
    }
}*/
