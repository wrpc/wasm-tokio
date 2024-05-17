use ::core::future::Future;

use tokio::io::{AsyncRead, AsyncReadExt as _, AsyncWrite, AsyncWriteExt as _};

pub trait AsyncReadCore: AsyncRead {
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(level = "trace", ret, skip_all, fields(ty = "u8"))
    )]
    fn read_u8_leb128(&mut self) -> impl Future<Output = std::io::Result<u8>>
    where
        Self: Unpin,
    {
        async {
            let mut x = 0;
            let mut s = 0;
            for _ in 0..2 {
                let b = self.read_u8().await?;
                if b < 0x80 {
                    if s == 14 && b > 0x01 {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "varint overflows an 8-bit integer",
                        ));
                    }
                    return Ok(x | b << s);
                }
                x |= (b & 0x7f) << s;
                s += 7;
            }
            Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "varint overflows an 8-bit integer",
            ))
        }
    }

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(level = "trace", ret, skip_all, fields(ty = "u16"))
    )]
    fn read_u16_leb128(&mut self) -> impl Future<Output = std::io::Result<u16>>
    where
        Self: Unpin,
    {
        async {
            let mut x = 0;
            let mut s = 0;
            for _ in 0..3 {
                let b: u16 = self.read_u8().await?.into();
                if b < 0x80 {
                    if s == 21 && b > 0x01 {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "varint overflows a 16-bit integer",
                        ));
                    }
                    return Ok(x | b << s);
                }
                x |= (b & 0x7f) << s;
                s += 7;
            }
            Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "varint overflows a 16-bit integer",
            ))
        }
    }

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(level = "trace", ret, skip_all, fields(ty = "u32"))
    )]
    fn read_u32_leb128(&mut self) -> impl Future<Output = std::io::Result<u32>>
    where
        Self: Unpin,
    {
        async {
            let mut x = 0;
            let mut s = 0;
            for _ in 0..5 {
                let b: u32 = self.read_u8().await?.into();
                if b < 0x80 {
                    if s == 28 && b > 0x01 {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "varint overflows a 32-bit integer",
                        ));
                    }
                    return Ok(x | b << s);
                }
                x |= (b & 0x7f) << s;
                s += 7;
            }
            Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "varint overflows a 32-bit integer",
            ))
        }
    }

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(level = "trace", ret, skip_all, fields(ty = "u64"))
    )]
    fn read_u64_leb128(&mut self) -> impl Future<Output = std::io::Result<u64>>
    where
        Self: Unpin,
    {
        async {
            let mut x = 0;
            let mut s = 0;
            for _ in 0..10 {
                let b: u64 = self.read_u8().await?.into();
                if b < 0x80 {
                    if s == 63 && b > 0x01 {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "varint overflows a 64-bit integer",
                        ));
                    }
                    return Ok(x | b << s);
                }
                x |= (b & 0x7f) << s;
                s += 7;
            }
            Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "varint overflows a 64-bit integer",
            ))
        }
    }

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(level = "trace", ret, skip_all, fields(ty = "string"))
    )]
    fn read_core_string(&mut self, s: &mut String) -> impl Future<Output = std::io::Result<()>>
    where
        Self: Unpin,
    {
        async {
            let n = self.read_u32_leb128().await?;
            self.take(n.into()).read_to_string(s).await?;
            Ok(())
        }
    }
}

impl<T: AsyncRead> AsyncReadCore for T {}

pub trait AsyncWriteCore: AsyncWrite {
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(level = "trace", ret, skip_all, fields(ty = "u8"))
    )]
    fn write_u8_leb128(&mut self, mut x: u8) -> impl Future<Output = std::io::Result<usize>>
    where
        Self: Unpin,
    {
        async {
            let mut buf = [0; 2];
            let mut i = 0;
            while x >= 0x80 {
                buf[i] = (x as u8) | 0x80;
                x >>= 7;
                i += 1;
            }
            buf[i] = x;
            self.write(&buf[..=i]).await
        }
    }

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(level = "trace", ret, skip_all, fields(ty = "u16"))
    )]
    fn write_u16_leb128(&mut self, mut x: u16) -> impl Future<Output = std::io::Result<usize>>
    where
        Self: Unpin,
    {
        async {
            let mut buf = [0; 3];
            let mut i = 0;
            while x >= 0x80 {
                buf[i] = (x as u8) | 0x80;
                x >>= 7;
                i += 1;
            }
            buf[i] = x as u8;
            self.write(&buf[..=i]).await
        }
    }

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(level = "trace", ret, skip_all, fields(ty = "u32"))
    )]
    fn write_u32_leb128(&mut self, mut x: u32) -> impl Future<Output = std::io::Result<usize>>
    where
        Self: Unpin,
    {
        async {
            let mut buf = [0; 5];
            let mut i = 0;
            while x >= 0x80 {
                buf[i] = (x as u8) | 0x80;
                x >>= 7;
                i += 1;
            }
            buf[i] = x as u8;
            self.write(&buf[..=i]).await
        }
    }

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(level = "trace", ret, skip_all, fields(ty = "u64"))
    )]
    fn write_u64_leb128(&mut self, mut x: u64) -> impl Future<Output = std::io::Result<usize>>
    where
        Self: Unpin,
    {
        async {
            let mut buf = [0; 10];
            let mut i = 0;
            while x >= 0x80 {
                buf[i] = (x as u8) | 0x80;
                x >>= 7;
                i += 1;
            }
            buf[i] = x as u8;
            self.write(&buf[..=i]).await
        }
    }

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(level = "trace", ret, skip_all, fields(ty = "string"))
    )]
    fn write_core_string(&mut self, s: &str) -> impl Future<Output = std::io::Result<()>>
    where
        Self: Unpin,
    {
        async {
            let n = s
                .len()
                .try_into()
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;
            self.write_u32_leb128(n).await?;
            self.write(s.as_bytes()).await?;
            Ok(())
        }
    }
}

impl<T: AsyncWrite> AsyncWriteCore for T {}

#[cfg(test)]
mod tests {
    use super::*;

    // LEB128 examples taken from https://en.wikipedia.org/wiki/LEB128

    #[tokio::test]
    async fn unsigned_leb128() {
        const ENCODED: [u8; 3] = [0xe5, 0x8e, 0x26];

        let v = ENCODED
            .as_slice()
            .read_u32_leb128()
            .await
            .expect("failed to read u32");
        assert_eq!(v, 624485);

        let v = ENCODED
            .as_slice()
            .read_u64_leb128()
            .await
            .expect("failed to read u64");
        assert_eq!(v, 624485);

        let mut buf = vec![];
        let n = buf
            .write_u32_leb128(624485)
            .await
            .expect("failed to write u32");
        assert_eq!(n, 3);
        assert_eq!(buf, ENCODED);

        let mut buf = vec![];
        let n = buf
            .write_u64_leb128(624485)
            .await
            .expect("failed to write u64");
        assert_eq!(n, 3);
        assert_eq!(buf, ENCODED);
    }

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
