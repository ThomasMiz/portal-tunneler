use std::{
    io::{self, Error, ErrorKind},
    num::NonZeroU16,
};

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use super::{ByteRead, ByteWrite};

impl ByteWrite for NonZeroU16 {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> io::Result<()> {
        writer.write_u16(self.get()).await
    }
}

impl ByteRead for NonZeroU16 {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> io::Result<Self> {
        NonZeroU16::new(reader.read_u16().await?).ok_or_else(|| Error::new(ErrorKind::InvalidData, "A NonZeroU16 was zero"))
    }
}
