use ::core::future::Future;

use tokio::io::{AsyncRead, AsyncReadExt as _, AsyncWrite, AsyncWriteExt as _};
use tokio_util::bytes::{BufMut as _, BytesMut};
use tokio_util::codec::Encoder;

pub trait AsyncReadCore: AsyncRead {
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(level = "trace", ret, skip_all, fields(ty = "u8"))
    )]
    fn read_u8_leb128(&mut self) -> impl Future<Output = std::io::Result<u8>>
    where
        Self: Unpin,
    {
        async move {
            let mut x = 0;
            let mut s = 0u8;
            for _ in 0..2 {
                let b = self.read_u8().await?;
                if s == 7 && b > 0x01 {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "varint overflows an 8-bit integer",
                    ));
                }
                if b < 0x80 {
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
        async move {
            let mut x = 0;
            let mut s = 0u8;
            for _ in 0..3 {
                let b: u16 = self.read_u8().await?.into();
                if s == 14 && b > 0x03 {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "varint overflows a 16-bit integer",
                    ));
                }
                if b < 0x80 {
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
        async move {
            let mut x = 0;
            let mut s = 0u8;
            for _ in 0..5 {
                let b: u32 = self.read_u8().await?.into();
                if s == 28 && b > 0x0f {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "varint overflows a 32-bit integer",
                    ));
                }
                if b < 0x80 {
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
        async move {
            let mut x = 0;
            let mut s = 0u8;
            for _ in 0..10 {
                let b: u64 = self.read_u8().await?.into();
                if s == 63 && b > 0x01 {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "varint overflows a 64-bit integer",
                    ));
                }
                if b < 0x80 {
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
        tracing::instrument(level = "trace", ret, skip_all, fields(ty = "u128"))
    )]
    fn read_u128_leb128(&mut self) -> impl Future<Output = std::io::Result<u128>>
    where
        Self: Unpin,
    {
        async move {
            let mut x = 0;
            let mut s = 0u8;
            for _ in 0..19 {
                let b: u128 = self.read_u8().await?.into();
                if s == 126 && b > 0x03 {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "varint overflows a 128-bit integer",
                    ));
                }
                if b < 0x80 {
                    return Ok(x | b << s);
                }
                x |= (b & 0x7f) << s;
                s += 7;
            }
            Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "varint overflows a 128-bit integer",
            ))
        }
    }

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(level = "trace", ret, skip_all, fields(ty = "uN"))
    )]
    fn read_core_unsigned_u8(&mut self, n: u8) -> impl Future<Output = std::io::Result<u8>>
    where
        Self: Unpin,
    {
        async move {
            debug_assert!(n <= 8);
            let max = (n / 7) + 1;
            let mut x = 0u8;
            let mut s = 0u8;
            for _ in 0..max {
                let b: u8 = self.read_u8().await?;
                if s == (n / 7) * 7 && b > n % 7 {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!("varint overflows a {n}-bit integer"),
                    ));
                }
                if b < 0x80 {
                    x |= b << s;
                    return Ok(x);
                }
                x |= (b & 0x7f) << s;
                s += 7;
            }
            Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("varint overflows a {n}-bit integer"),
            ))
        }
    }

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(level = "trace", ret, skip_all, fields(ty = "uN"))
    )]
    fn read_core_unsigned_u16(&mut self, n: u8) -> impl Future<Output = std::io::Result<u16>>
    where
        Self: Unpin,
    {
        async move {
            debug_assert!(n <= 16);
            let max = (n / 7) + 1;
            let mut x = 0;
            let mut s = 0u8;
            for _ in 0..max {
                let b = self.read_u8().await?;
                if s == (n / 7) * 7 && b > n % 7 {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!("varint overflows a {n}-bit integer"),
                    ));
                }
                let b: u16 = b.into();
                if b < 0x80 {
                    x |= b << s;
                    return Ok(x);
                }
                x |= (b & 0x7f) << s;
                s += 7;
            }
            Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("varint overflows a {n}-bit integer"),
            ))
        }
    }

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(level = "trace", ret, skip_all, fields(ty = "uN"))
    )]
    fn read_core_unsigned_u32(&mut self, n: u8) -> impl Future<Output = std::io::Result<u32>>
    where
        Self: Unpin,
    {
        async move {
            debug_assert!(n <= 32);
            let max = (n / 7) + 1;
            let mut x = 0;
            let mut s = 0u8;
            for _ in 0..max {
                let b = self.read_u8().await?;
                if s == (n / 7) * 7 && b > n % 7 {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!("varint overflows a {n}-bit integer"),
                    ));
                }
                let b: u32 = b.into();
                if b < 0x80 {
                    x |= b << s;
                    return Ok(x);
                }
                x |= (b & 0x7f) << s;
                s += 7;
            }
            Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("varint overflows a {n}-bit integer"),
            ))
        }
    }

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(level = "trace", ret, skip_all, fields(ty = "uN"))
    )]
    fn read_core_unsigned_u64(&mut self, n: u8) -> impl Future<Output = std::io::Result<u64>>
    where
        Self: Unpin,
    {
        async move {
            debug_assert!(n <= 64);
            let max = (n / 7) + 1;
            let mut x = 0;
            let mut s = 0u8;
            for _ in 0..max {
                let b = self.read_u8().await?;
                if s == (n / 7) * 7 && b > n % 7 {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!("varint overflows a {n}-bit integer"),
                    ));
                }
                let b: u64 = b.into();
                if b < 0x80 {
                    x |= b << s;
                    return Ok(x);
                }
                x |= (b & 0x7f) << s;
                s += 7;
            }
            Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("varint overflows a {n}-bit integer"),
            ))
        }
    }

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(level = "trace", ret, skip_all, fields(ty = "uN"))
    )]
    fn read_core_unsigned_u128(&mut self, n: u8) -> impl Future<Output = std::io::Result<u128>>
    where
        Self: Unpin,
    {
        async move {
            debug_assert!(n <= 128);
            let max = (n / 7) + 1;
            let mut x = 0;
            let mut s = 0u8;
            for _ in 0..max {
                let b = self.read_u8().await?;
                if s == (n / 7) * 7 && b > n % 7 {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!("varint overflows a {n}-bit integer"),
                    ));
                }
                let b: u128 = b.into();
                if b < 0x80 {
                    x |= b << s;
                    return Ok(x);
                }
                x |= (b & 0x7f) << s;
                s += 7;
            }
            Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("varint overflows a {n}-bit integer"),
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
        async move {
            let n = self.read_u32_leb128().await?;
            s.reserve(n.try_into().unwrap_or(usize::MAX));
            self.take(n.into()).read_to_string(s).await?;
            Ok(())
        }
    }
}

impl<T: AsyncRead> AsyncReadCore for T {}

pub fn put_u8_leb128(buf: &mut [u8; 2], mut x: u8) -> &mut [u8] {
    let mut i = 0;
    while x >= 0x80 {
        buf[i] = (x as u8) | 0x80;
        x >>= 7;
        i += 1;
    }
    buf[i] = x;
    &mut buf[..=i]
}

pub fn put_u16_leb128(buf: &mut [u8; 3], mut x: u16) -> &mut [u8] {
    let mut i = 0;
    while x >= 0x80 {
        buf[i] = (x as u8) | 0x80;
        x >>= 7;
        i += 1;
    }
    buf[i] = x as u8;
    &mut buf[..=i]
}

pub fn put_u32_leb128(buf: &mut [u8; 5], mut x: u32) -> &mut [u8] {
    let mut i = 0;
    while x >= 0x80 {
        buf[i] = (x as u8) | 0x80;
        x >>= 7;
        i += 1;
    }
    buf[i] = x as u8;
    &mut buf[..=i]
}

pub fn put_u64_leb128(buf: &mut [u8; 10], mut x: u64) -> &mut [u8] {
    let mut i = 0;
    while x >= 0x80 {
        buf[i] = (x as u8) | 0x80;
        x >>= 7;
        i += 1;
    }
    buf[i] = x as u8;
    &mut buf[..=i]
}

pub fn put_u128_leb128(buf: &mut [u8; 19], mut x: u128) -> &mut [u8] {
    let mut i = 0;
    while x >= 0x80 {
        buf[i] = (x as u8) | 0x80;
        x >>= 7;
        i += 1;
    }
    buf[i] = x as u8;
    &mut buf[..=i]
}

pub trait AsyncWriteCore: AsyncWrite {
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(level = "trace", ret, skip_all, fields(ty = "u8"))
    )]
    fn write_u8_leb128(&mut self, x: u8) -> impl Future<Output = std::io::Result<()>>
    where
        Self: Unpin,
    {
        async move {
            self.write_all(put_u8_leb128(&mut Default::default(), x))
                .await
        }
    }

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(level = "trace", ret, skip_all, fields(ty = "u16"))
    )]
    fn write_u16_leb128(&mut self, x: u16) -> impl Future<Output = std::io::Result<()>>
    where
        Self: Unpin,
    {
        async move {
            self.write_all(put_u16_leb128(&mut Default::default(), x))
                .await
        }
    }

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(level = "trace", ret, skip_all, fields(ty = "u32"))
    )]
    fn write_u32_leb128(&mut self, x: u32) -> impl Future<Output = std::io::Result<()>>
    where
        Self: Unpin,
    {
        async move {
            self.write_all(put_u32_leb128(&mut Default::default(), x))
                .await
        }
    }

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(level = "trace", ret, skip_all, fields(ty = "u64"))
    )]
    fn write_u64_leb128(&mut self, x: u64) -> impl Future<Output = std::io::Result<()>>
    where
        Self: Unpin,
    {
        async move {
            self.write_all(put_u64_leb128(&mut Default::default(), x))
                .await
        }
    }

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(level = "trace", ret, skip_all, fields(ty = "u128"))
    )]
    fn write_u128_leb128(&mut self, x: u128) -> impl Future<Output = std::io::Result<()>>
    where
        Self: Unpin,
    {
        async move {
            self.write_all(put_u128_leb128(&mut Default::default(), x))
                .await
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
        async move {
            let mut buf = BytesMut::with_capacity(5usize.saturating_add(s.len()));
            CoreStringEncoder.encode(s, &mut buf)?;
            self.write_all(&buf).await
        }
    }
}

impl<T: AsyncWrite> AsyncWriteCore for T {}

pub struct Leb128Encoder;

impl Encoder<u8> for Leb128Encoder {
    type Error = std::io::Error;

    fn encode(&mut self, x: u8, dst: &mut BytesMut) -> Result<(), Self::Error> {
        dst.extend_from_slice(put_u8_leb128(&mut Default::default(), x));
        Ok(())
    }
}

impl Encoder<u16> for Leb128Encoder {
    type Error = std::io::Error;

    fn encode(&mut self, x: u16, dst: &mut BytesMut) -> Result<(), Self::Error> {
        dst.extend_from_slice(put_u16_leb128(&mut Default::default(), x));
        Ok(())
    }
}

impl Encoder<u32> for Leb128Encoder {
    type Error = std::io::Error;

    fn encode(&mut self, x: u32, dst: &mut BytesMut) -> Result<(), Self::Error> {
        dst.extend_from_slice(put_u32_leb128(&mut Default::default(), x));
        Ok(())
    }
}

impl Encoder<u64> for Leb128Encoder {
    type Error = std::io::Error;

    fn encode(&mut self, x: u64, dst: &mut BytesMut) -> Result<(), Self::Error> {
        dst.extend_from_slice(put_u64_leb128(&mut Default::default(), x));
        Ok(())
    }
}

impl Encoder<u128> for Leb128Encoder {
    type Error = std::io::Error;

    fn encode(&mut self, x: u128, dst: &mut BytesMut) -> Result<(), Self::Error> {
        dst.extend_from_slice(put_u128_leb128(&mut Default::default(), x));
        Ok(())
    }
}

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
    async fn unsigned_leb128() {
        const ENCODED: [u8; 3] = [0xe5, 0x8e, 0x26];

        let v = ENCODED
            .as_slice()
            .read_u32_leb128()
            .await
            .expect("failed to read u32");
        assert_eq!(v, 624_485);

        let v = ENCODED
            .as_slice()
            .read_u64_leb128()
            .await
            .expect("failed to read u64");
        assert_eq!(v, 624_485);

        let mut buf = vec![];
        buf.write_u32_leb128(624_485)
            .await
            .expect("failed to write u32");
        assert_eq!(buf, ENCODED);

        let mut buf = vec![];
        buf.write_u64_leb128(624_485)
            .await
            .expect("failed to write u64");
        assert_eq!(buf, ENCODED);

        let v = [0xff, 0x01]
            .as_slice()
            .read_u8_leb128()
            .await
            .expect("failed to read u8");
        assert_eq!(v, u8::MAX);

        let v = [0x7f]
            .as_slice()
            .read_u8_leb128()
            .await
            .expect("failed to read u8");
        assert_eq!(v, 0x7f);

        let v = [0xff, 0x00]
            .as_slice()
            .read_u8_leb128()
            .await
            .expect("failed to read u8");
        assert_eq!(v, 0x7f);

        [0xff, 0x02]
            .as_slice()
            .read_u8_leb128()
            .await
            .expect_err("u8 read should have failed, since it encoded 9 bits");

        let v = [0xff, 0xff, 0x01]
            .as_slice()
            .read_u16_leb128()
            .await
            .expect("failed to read u16");
        assert_eq!(v, 0x7fff);

        let v = [0xff, 0xff, 0x02]
            .as_slice()
            .read_u16_leb128()
            .await
            .expect("failed to read u16");
        assert_eq!(v, 0xbfff);

        let v = [0xff, 0xff, 0x03]
            .as_slice()
            .read_u16_leb128()
            .await
            .expect("failed to read u16");
        assert_eq!(v, u16::MAX);

        [0xff, 0xff, 0x04]
            .as_slice()
            .read_u16_leb128()
            .await
            .expect_err("u16 read should have failed, since it encoded 17 bits");

        let v = [0xff, 0xff, 0xff, 0xff, 0x0f]
            .as_slice()
            .read_u32_leb128()
            .await
            .expect("failed to read u32");
        assert_eq!(v, u32::MAX);

        [0xff, 0xff, 0xff, 0xff, 0x10]
            .as_slice()
            .read_u32_leb128()
            .await
            .expect_err("u32 read should have failed, since it encoded 33 bits");

        let v = [0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x01]
            .as_slice()
            .read_u64_leb128()
            .await
            .expect("failed to read u64");
        assert_eq!(v, u64::MAX);

        [0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x02]
            .as_slice()
            .read_u64_leb128()
            .await
            .expect_err("u64 read should have failed, since it encoded 65 bits");

        let v = [
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff, 0xff, 0xff, 0xff, 0x03,
        ]
        .as_slice()
        .read_u128_leb128()
        .await
        .expect("failed to read u128");
        assert_eq!(v, u128::MAX);

        [
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff, 0xff, 0xff, 0xff, 0x04,
        ]
        .as_slice()
        .read_u128_leb128()
        .await
        .expect_err("u128 read should have failed, since it encoded 129 bits");
    }

    #[tokio::test]
    async fn unsigned() {
        let v = [0x01u8]
            .as_slice()
            .read_core_unsigned_u8(2)
            .await
            .expect("failed to read u2");
        assert_eq!(v, 1);

        let v = [0x02]
            .as_slice()
            .read_core_unsigned_u8(2)
            .await
            .expect("failed to read u2");
        assert_eq!(v, 2);

        [0b100]
            .as_slice()
            .read_core_unsigned_u8(2)
            .await
            .expect_err("u2 read should have failed, since it encoded 3 bits");

        [0x80, 0x80, 0x01]
            .as_slice()
            .read_core_unsigned_u16(9)
            .await
            .expect_err("u9 read should have failed, since it used over 9 bits");

        let v = [0x80, 0x80, 0x80, 0x80, 0x80, 0x01]
            .as_slice()
            .read_core_unsigned_u64(64)
            .await
            .expect("failed to read u64");
        assert_eq!(v, 0b1000_0000_0000_0000_0000_0000_0000_0000_0000);
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
