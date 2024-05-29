use ::core::future::Future;

use leb128_tokio::{AsyncReadLeb128, Leb128Encoder};
use tokio::io::{AsyncRead, AsyncReadExt as _, AsyncWrite, AsyncWriteExt as _};
use tokio_util::bytes::{BufMut as _, BytesMut};
use tokio_util::codec::Encoder;

pub trait AsyncReadCore: AsyncRead {
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(level = "trace", ret, skip_all, fields(ty = "string"))
    )]
    fn read_core_string(&mut self, s: &mut String) -> impl Future<Output = std::io::Result<()>>
    where
        Self: Unpin + Sized,
    {
        async move {
            let n = self.read_u32_leb128().await?;
            s.reserve(n.try_into().unwrap_or(usize::MAX));
            self.take(n.into()).read_to_string(s).await?;
            Ok(())
        }
    }
}

impl<T: AsyncRead> AsyncReadCore for T {}

pub trait AsyncWriteCore: AsyncWrite {
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(level = "trace", ret, skip_all, fields(ty = "string"))
    )]
    fn write_core_string(&mut self, s: &str) -> impl Future<Output = std::io::Result<()>>
    where
        Self: Unpin,
    {
        async move {
            let mut buf = BytesMut::with_capacity(5usize.saturating_add(s.len()));
            CoreStringEncoder.encode(s, &mut buf)?;
            self.write_all(&buf).await
        }
    }
}

impl<T: AsyncWrite> AsyncWriteCore for T {}

pub struct CoreStringEncoder;

impl Encoder<&str> for CoreStringEncoder {
    type Error = std::io::Error;

    fn encode(&mut self, item: &str, dst: &mut BytesMut) -> Result<(), Self::Error> {
        let len = item.len();
        let n: u32 = len
            .try_into()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;
        dst.reserve(len + 5 - n.leading_zeros() as usize / 7);
        Leb128Encoder.encode(n, dst)?;
        dst.put(item.as_bytes());
        Ok(())
    }
}

impl Encoder<String> for CoreStringEncoder {
    type Error = std::io::Error;

    fn encode(&mut self, item: String, dst: &mut BytesMut) -> Result<(), Self::Error> {
        self.encode(item.as_str(), dst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn string() {
        const ENCODED: [u8; 5] = [0x04, b't', b'e', b's', b't'];

        let mut s = String::new();
        ENCODED
            .as_slice()
            .read_core_string(&mut s)
            .await
            .expect("failed to read string");
        assert_eq!(s, "test");

        let mut buf = vec![];
        buf.write_core_string("test")
            .await
            .expect("failed to write string");
        assert_eq!(buf, ENCODED);
    }
}
