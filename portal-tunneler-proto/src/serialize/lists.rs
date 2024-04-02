use std::io::{self, Error, ErrorKind};

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use super::{ByteRead, ByteWrite};

impl<T: ByteWrite> ByteWrite for &[T] {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> io::Result<()> {
        let len = self.len();
        if len > u16::MAX as usize {
            return Err(Error::new(ErrorKind::InvalidData, "List is too long (>= 64K)"));
        }

        let len = len as u16;
        writer.write_u16(len).await?;
        for ele in self.iter() {
            ele.write(writer).await?;
        }

        Ok(())
    }
}

impl<T: ByteRead> ByteRead for Vec<T> {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> io::Result<Self> {
        let len = reader.read_u16().await? as usize;

        let mut v = Vec::with_capacity(len);
        for _ in 0..len {
            v.push(T::read(reader).await?);
        }

        Ok(v)
    }
}

/// A type that wraps a `&[T]` and implements [`ByteWrite`] for easily writing lists whose max
/// length is 255 elements.
pub struct SmallWriteList<'a, T>(pub &'a [T]);

impl<'a, T: ByteWrite> ByteWrite for SmallWriteList<'a, T> {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> io::Result<()> {
        let len = self.0.len();
        if len > u8::MAX as usize {
            return Err(Error::new(ErrorKind::InvalidData, "Small list is too long (>= 256)"));
        }

        let len = len as u8;
        writer.write_u8(len).await?;
        for ele in self.0.iter() {
            ele.write(writer).await?;
        }

        Ok(())
    }
}
/// A type that wraps a [`Vec<T>`] and implements [`ByteRead`] for easily reading lists whose max
/// length is 255 elements.
pub struct SmallReadList<T>(pub Vec<T>);

impl<T: ByteRead> ByteRead for SmallReadList<T> {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> io::Result<Self> {
        let len = reader.read_u8().await? as usize;

        let mut v = Vec::with_capacity(len);
        for _ in 0..len {
            v.push(T::read(reader).await?);
        }

        Ok(SmallReadList(v))
    }
}
