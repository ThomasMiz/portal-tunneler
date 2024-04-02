use std::io;

use tokio::io::{AsyncRead, AsyncWrite};

use super::{ByteRead, ByteWrite};

impl ByteWrite for () {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, _: &mut W) -> io::Result<()> {
        Ok(())
    }
}

impl ByteRead for () {
    async fn read<R: AsyncRead + Unpin + ?Sized>(_: &mut R) -> io::Result<Self> {
        Ok(())
    }
}

impl<T0: ByteWrite, T1: ByteWrite> ByteWrite for (T0, T1) {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> io::Result<()> {
        self.0.write(writer).await?;
        self.1.write(writer).await
    }
}

impl<T0: ByteRead, T1: ByteRead> ByteRead for (T0, T1) {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> io::Result<Self> {
        Ok((T0::read(reader).await?, T1::read(reader).await?))
    }
}

impl<T0: ByteWrite, T1: ByteWrite, T2: ByteWrite> ByteWrite for (T0, T1, T2) {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> io::Result<()> {
        self.0.write(writer).await?;
        self.1.write(writer).await?;
        self.2.write(writer).await
    }
}

impl<T0: ByteRead, T1: ByteRead, T2: ByteRead> ByteRead for (T0, T1, T2) {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> io::Result<Self> {
        Ok((T0::read(reader).await?, T1::read(reader).await?, T2::read(reader).await?))
    }
}

impl<T0: ByteWrite, T1: ByteWrite, T2: ByteWrite, T3: ByteWrite> ByteWrite for (T0, T1, T2, T3) {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> io::Result<()> {
        self.0.write(writer).await?;
        self.1.write(writer).await?;
        self.2.write(writer).await?;
        self.3.write(writer).await
    }
}

impl<T0: ByteRead, T1: ByteRead, T2: ByteRead, T3: ByteRead> ByteRead for (T0, T1, T2, T3) {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> io::Result<Self> {
        Ok((
            T0::read(reader).await?,
            T1::read(reader).await?,
            T2::read(reader).await?,
            T3::read(reader).await?,
        ))
    }
}

impl<T0: ByteWrite, T1: ByteWrite, T2: ByteWrite, T3: ByteWrite, T4: ByteWrite> ByteWrite for (T0, T1, T2, T3, T4) {
    async fn write<W: AsyncWrite + Unpin + ?Sized>(&self, writer: &mut W) -> io::Result<()> {
        self.0.write(writer).await?;
        self.1.write(writer).await?;
        self.2.write(writer).await?;
        self.3.write(writer).await?;
        self.4.write(writer).await
    }
}

impl<T0: ByteRead, T1: ByteRead, T2: ByteRead, T3: ByteRead, T4: ByteRead> ByteRead for (T0, T1, T2, T3, T4) {
    async fn read<R: AsyncRead + Unpin + ?Sized>(reader: &mut R) -> io::Result<Self> {
        Ok((
            T0::read(reader).await?,
            T1::read(reader).await?,
            T2::read(reader).await?,
            T3::read(reader).await?,
            T4::read(reader).await?,
        ))
    }
}
