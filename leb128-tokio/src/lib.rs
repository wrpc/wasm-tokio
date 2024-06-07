use ::core::future::Future;
use core::fmt::Display;
use core::marker::PhantomData;

use tokio::io::{AsyncRead, AsyncReadExt as _, AsyncWrite, AsyncWriteExt as _};
use tokio_util::bytes::BytesMut;
use tokio_util::codec::{Decoder, Encoder};

/// Error returned for overflows decoding statically-sized integers
#[derive(Debug)]
pub struct Overflow<const N: usize>;

impl Display for Overflow<8> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "varint overflows an 8-bit integer")
    }
}

impl std::error::Error for Overflow<8> {}

impl Display for Overflow<16> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "varint overflows a 16-bit integer")
    }
}

impl std::error::Error for Overflow<16> {}

impl Display for Overflow<32> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "varint overflows a 32-bit integer")
    }
}

impl std::error::Error for Overflow<32> {}

impl Display for Overflow<64> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "varint overflows a 64-bit integer")
    }
}

impl std::error::Error for Overflow<64> {}

impl Display for Overflow<128> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "varint overflows a 128-bit integer")
    }
}

impl std::error::Error for Overflow<128> {}

/// Error returned for overflows decoding variable size integers
#[derive(Debug)]
pub struct OverflowVar(u8);

impl Display for OverflowVar {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "varint overflows a {}-bit integer", self.0)
    }
}

impl std::error::Error for OverflowVar {}

fn invalid_data(err: impl Sync + Send + std::error::Error + 'static) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, err)
}

pub trait AsyncReadLeb128: AsyncRead {
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
                    return Err(invalid_data(Overflow::<8>));
                }
                x |= (b & 0x7f) << s;
                if b & 0x80 == 0 {
                    return Ok(x);
                }
                s += 7;
            }
            Err(invalid_data(Overflow::<8>))
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
                let b = self.read_u8().await?;
                if s == 14 && b > 0x03 {
                    return Err(invalid_data(Overflow::<16>));
                }
                x |= (u16::from(b) & 0x7f) << s;
                if b & 0x80 == 0 {
                    return Ok(x);
                }
                s += 7;
            }
            Err(invalid_data(Overflow::<16>))
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
                let b = self.read_u8().await?;
                if s == 28 && b > 0x0f {
                    return Err(invalid_data(Overflow::<32>));
                }
                x |= (u32::from(b) & 0x7f) << s;
                if b & 0x80 == 0 {
                    return Ok(x);
                }
                s += 7;
            }
            Err(invalid_data(Overflow::<32>))
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
                let b = self.read_u8().await?;
                if s == 63 && b > 0x01 {
                    return Err(invalid_data(Overflow::<64>));
                }
                x |= (u64::from(b) & 0x7f) << s;
                if b & 0x80 == 0 {
                    return Ok(x);
                }
                s += 7;
            }
            Err(invalid_data(Overflow::<64>))
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
                let b = self.read_u8().await?;
                if s == 126 && b > 0x03 {
                    return Err(invalid_data(Overflow::<128>));
                }
                x |= (u128::from(b) & 0x7f) << s;
                if b & 0x80 == 0 {
                    return Ok(x);
                }
                s += 7;
            }
            Err(invalid_data(Overflow::<128>))
        }
    }

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(level = "trace", ret, skip_all, fields(ty = "uvarint", n))
    )]
    fn read_var_u8_leb128(&mut self, n: u8) -> impl Future<Output = std::io::Result<u8>>
    where
        Self: Unpin,
    {
        async move {
            if n == 8 {
                return self.read_u8_leb128().await;
            }
            debug_assert!(n <= 8);
            let max = (n / 7) + 1;
            let mut x = 0u8;
            let mut s = 0u8;
            for _ in 0..max {
                let b = self.read_u8().await?;
                if s == (n / 7) * 7 && b > n % 7 {
                    return Err(invalid_data(OverflowVar(n)));
                }
                x |= (b & 0x7f) << s;
                if b & 0x80 == 0 {
                    return Ok(x);
                }
                s += 7;
            }
            Err(invalid_data(OverflowVar(n)))
        }
    }

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(level = "trace", ret, skip_all, fields(ty = "uvarint", n))
    )]
    fn read_var_u16_leb128(&mut self, n: u8) -> impl Future<Output = std::io::Result<u16>>
    where
        Self: Unpin,
    {
        async move {
            match n {
                8 => return self.read_u8_leb128().await.map(Into::into),
                16 => return self.read_u16_leb128().await,
                _ => {}
            }
            debug_assert!(n <= 16);
            let max = (n / 7) + 1;
            let mut x = 0;
            let mut s = 0u8;
            for _ in 0..max {
                let b = self.read_u8().await?;
                if s == (n / 7) * 7 && b > n % 7 {
                    return Err(invalid_data(OverflowVar(n)));
                }
                x |= (u16::from(b) & 0x7f) << s;
                if b & 0x80 == 0 {
                    return Ok(x);
                }
                s += 7;
            }
            Err(invalid_data(OverflowVar(n)))
        }
    }

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(level = "trace", ret, skip_all, fields(ty = "uvarint", n))
    )]
    fn read_var_u32_leb128(&mut self, n: u8) -> impl Future<Output = std::io::Result<u32>>
    where
        Self: Unpin,
    {
        async move {
            match n {
                8 => return self.read_u8_leb128().await.map(Into::into),
                16 => return self.read_u16_leb128().await.map(Into::into),
                32 => return self.read_u32_leb128().await,
                _ => {}
            }
            debug_assert!(n <= 32);
            let max = (n / 7) + 1;
            let mut x = 0;
            let mut s = 0u8;
            for _ in 0..max {
                let b = self.read_u8().await?;
                if s == (n / 7) * 7 && b > n % 7 {
                    return Err(invalid_data(OverflowVar(n)));
                }
                x |= (u32::from(b) & 0x7f) << s;
                if b & 0x80 == 0 {
                    return Ok(x);
                }
                s += 7;
            }
            Err(invalid_data(OverflowVar(n)))
        }
    }

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(level = "trace", ret, skip_all, fields(ty = "uvarint", n))
    )]
    fn read_var_u64_leb128(&mut self, n: u8) -> impl Future<Output = std::io::Result<u64>>
    where
        Self: Unpin,
    {
        async move {
            match n {
                8 => return self.read_u8_leb128().await.map(Into::into),
                16 => return self.read_u16_leb128().await.map(Into::into),
                32 => return self.read_u32_leb128().await.map(Into::into),
                64 => return self.read_u64_leb128().await,
                _ => {}
            }
            debug_assert!(n <= 64);
            let max = (n / 7) + 1;
            let mut x = 0;
            let mut s = 0u8;
            for _ in 0..max {
                let b = self.read_u8().await?;
                if s == (n / 7) * 7 && b > n % 7 {
                    return Err(invalid_data(OverflowVar(n)));
                }
                x |= (u64::from(b) & 0x7f) << s;
                if b & 0x80 == 0 {
                    return Ok(x);
                }
                s += 7;
            }
            Err(invalid_data(OverflowVar(n)))
        }
    }

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(level = "trace", ret, skip_all, fields(ty = "uvarint", n))
    )]
    fn read_var_u128_leb128(&mut self, n: u8) -> impl Future<Output = std::io::Result<u128>>
    where
        Self: Unpin,
    {
        async move {
            match n {
                8 => return self.read_u8_leb128().await.map(Into::into),
                16 => return self.read_u16_leb128().await.map(Into::into),
                32 => return self.read_u32_leb128().await.map(Into::into),
                64 => return self.read_u64_leb128().await.map(Into::into),
                128 => return self.read_u128_leb128().await,
                _ => {}
            }
            debug_assert!(n <= 128);
            let max = (n / 7) + 1;
            let mut x = 0;
            let mut s = 0u8;
            for _ in 0..max {
                let b = self.read_u8().await?;
                if s == (n / 7) * 7 && b > n % 7 {
                    return Err(invalid_data(OverflowVar(n)));
                }
                x |= (u128::from(b) & 0x7f) << s;
                if b & 0x80 == 0 {
                    return Ok(x);
                }
                s += 7;
            }
            Err(invalid_data(OverflowVar(n)))
        }
    }

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(level = "trace", ret, skip_all, fields(ty = "i8"))
    )]
    fn read_i8_leb128(&mut self) -> impl Future<Output = std::io::Result<i8>>
    where
        Self: Unpin,
    {
        async move {
            let mut x = 0;
            let mut s = 0u8;
            for _ in 0..2 {
                let b = self.read_u8().await?;
                if s == 7 && b > 0x01 {
                    return Err(invalid_data(Overflow::<8>));
                }
                x |= ((b as i8) & 0x7f) << s;
                s += 7;
                if b & 0x80 == 0 {
                    if s != 14 && b & 0x40 != 0 {
                        return Ok(x | !0 << s);
                    } else {
                        return Ok(x);
                    }
                }
            }
            Err(invalid_data(Overflow::<8>))
        }
    }

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(level = "trace", ret, skip_all, fields(ty = "i16"))
    )]
    fn read_i16_leb128(&mut self) -> impl Future<Output = std::io::Result<i16>>
    where
        Self: Unpin,
    {
        async move {
            let mut x = 0;
            let mut s = 0u8;
            for _ in 0..3 {
                let b = self.read_u8().await?;
                if s == 14 && b > 0x03 {
                    return Err(invalid_data(Overflow::<16>));
                }
                x |= (i16::from(b) & 0x7f) << s;
                s += 7;
                if b & 0x80 == 0 {
                    if s != 21 && b & 0x40 != 0 {
                        return Ok(x | !0 << s);
                    } else {
                        return Ok(x);
                    }
                }
            }
            Err(invalid_data(Overflow::<16>))
        }
    }

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(level = "trace", ret, skip_all, fields(ty = "i32"))
    )]
    fn read_i32_leb128(&mut self) -> impl Future<Output = std::io::Result<i32>>
    where
        Self: Unpin,
    {
        async move {
            let mut x = 0;
            let mut s = 0u8;
            for _ in 0..5 {
                let b = self.read_u8().await?;
                if s == 28 && b > 0x0f {
                    return Err(invalid_data(Overflow::<32>));
                }
                x |= (i32::from(b) & 0x7f) << s;
                s += 7;
                if b & 0x80 == 0 {
                    if s != 35 && b & 0x40 != 0 {
                        return Ok(x | !0 << s);
                    } else {
                        return Ok(x);
                    }
                }
            }
            Err(invalid_data(Overflow::<32>))
        }
    }

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(level = "trace", ret, skip_all, fields(ty = "i64"))
    )]
    fn read_i64_leb128(&mut self) -> impl Future<Output = std::io::Result<i64>>
    where
        Self: Unpin,
    {
        async move {
            let mut x = 0;
            let mut s = 0u8;
            for _ in 0..10 {
                let b = self.read_u8().await?;
                if s == 63 && b > 0x01 {
                    return Err(invalid_data(Overflow::<64>));
                }
                x |= (i64::from(b) & 0x7f) << s;
                s += 7;
                if b & 0x80 == 0 {
                    if s != 70 && b & 0x40 != 0 {
                        return Ok(x | !0 << s);
                    } else {
                        return Ok(x);
                    }
                }
            }
            Err(invalid_data(Overflow::<64>))
        }
    }

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(level = "trace", ret, skip_all, fields(ty = "i128"))
    )]
    fn read_i128_leb128(&mut self) -> impl Future<Output = std::io::Result<i128>>
    where
        Self: Unpin,
    {
        async move {
            let mut x = 0;
            let mut s = 0u8;
            for _ in 0..19 {
                let b = self.read_u8().await?;
                if s == 126 && b > 0x03 {
                    return Err(invalid_data(Overflow::<128>));
                }
                x |= (i128::from(b) & 0x7f) << s;
                s += 7;
                if b & 0x80 == 0 {
                    if s != 133 && b & 0x40 != 0 {
                        return Ok(x | !0 << s);
                    } else {
                        return Ok(x);
                    }
                }
            }
            Err(invalid_data(Overflow::<128>))
        }
    }
}

impl<T: AsyncRead> AsyncReadLeb128 for T {}

pub fn put_u8_leb128(buf: &mut [u8; 2], mut x: u8) -> &mut [u8] {
    let mut i = 0;
    while x >= 0x80 {
        buf[i] = x | 0x80;
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

pub fn put_i8_leb128(buf: &mut [u8; 2], mut x: i8) -> &mut [u8] {
    let mut i = 0;
    loop {
        let b = x as u8;
        x >>= 6;
        if x == 0 || x == !0 {
            buf[i] = b & 0x7f;
            return &mut buf[..=i];
        } else {
            buf[i] = b | 0x80;
            x >>= 1;
        }
        i += 1;
    }
}

pub fn put_i16_leb128(buf: &mut [u8; 3], mut x: i16) -> &mut [u8] {
    let mut i = 0;
    loop {
        let b = x as u8;
        x >>= 6;
        if x == 0 || x == !0 {
            buf[i] = b & 0x7f;
            return &mut buf[..=i];
        } else {
            buf[i] = b | 0x80;
            x >>= 1;
        }
        i += 1;
    }
}

pub fn put_i32_leb128(buf: &mut [u8; 5], mut x: i32) -> &mut [u8] {
    let mut i = 0;
    loop {
        let b = x as u8;
        x >>= 6;
        if x == 0 || x == !0 {
            buf[i] = b & 0x7f;
            return &mut buf[..=i];
        } else {
            buf[i] = b | 0x80;
            x >>= 1;
        }
        i += 1;
    }
}

pub fn put_i64_leb128(buf: &mut [u8; 10], mut x: i64) -> &mut [u8] {
    let mut i = 0;
    loop {
        let b = x as u8;
        x >>= 6;
        if x == 0 || x == !0 {
            buf[i] = b & 0x7f;
            return &mut buf[..=i];
        } else {
            buf[i] = b | 0x80;
            x >>= 1;
        }
        i += 1;
    }
}

pub fn put_i128_leb128(buf: &mut [u8; 19], mut x: i128) -> &mut [u8] {
    let mut i = 0;
    loop {
        let b = x as u8;
        x >>= 6;
        if x == 0 || x == !0 {
            buf[i] = b & 0x7f;
            return &mut buf[..=i];
        } else {
            buf[i] = b | 0x80;
            x >>= 1;
        }
        i += 1;
    }
}

pub trait AsyncWriteLeb128: AsyncWrite {
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
        tracing::instrument(level = "trace", ret, skip_all, fields(ty = "i8"))
    )]
    fn write_i8_leb128(&mut self, x: i8) -> impl Future<Output = std::io::Result<()>>
    where
        Self: Unpin,
    {
        async move {
            self.write_all(put_i8_leb128(&mut Default::default(), x))
                .await
        }
    }

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(level = "trace", ret, skip_all, fields(ty = "i16"))
    )]
    fn write_i16_leb128(&mut self, x: i16) -> impl Future<Output = std::io::Result<()>>
    where
        Self: Unpin,
    {
        async move {
            self.write_all(put_i16_leb128(&mut Default::default(), x))
                .await
        }
    }

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(level = "trace", ret, skip_all, fields(ty = "i32"))
    )]
    fn write_i32_leb128(&mut self, x: i32) -> impl Future<Output = std::io::Result<()>>
    where
        Self: Unpin,
    {
        async move {
            self.write_all(put_i32_leb128(&mut Default::default(), x))
                .await
        }
    }

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(level = "trace", ret, skip_all, fields(ty = "i64"))
    )]
    fn write_i64_leb128(&mut self, x: i64) -> impl Future<Output = std::io::Result<()>>
    where
        Self: Unpin,
    {
        async move {
            self.write_all(put_i64_leb128(&mut Default::default(), x))
                .await
        }
    }

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(level = "trace", ret, skip_all, fields(ty = "i128"))
    )]
    fn write_i128_leb128(&mut self, x: i128) -> impl Future<Output = std::io::Result<()>>
    where
        Self: Unpin,
    {
        async move {
            self.write_all(put_i128_leb128(&mut Default::default(), x))
                .await
        }
    }
}

impl<T: AsyncWrite> AsyncWriteLeb128 for T {}

pub struct Leb128DecoderU8;

impl Decoder for Leb128DecoderU8 {
    type Item = u8;
    type Error = std::io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        let mut x = 0;
        let mut s = 0u8;
        for i in 0..2 {
            let Some(b) = src.get(i) else {
                src.reserve(1);
                return Ok(None);
            };
            if s == 7 && *b > 0x01 {
                return Err(invalid_data(Overflow::<8>));
            }
            x |= (b & 0x7f) << s;
            if b & 0x80 == 0 {
                return Ok(Some(x));
            }
            s += 7;
        }
        Err(invalid_data(Overflow::<8>))
    }
}

pub struct Leb128DecoderU16;

impl Decoder for Leb128DecoderU16 {
    type Item = u16;
    type Error = std::io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        let mut x = 0;
        let mut s = 0u8;
        for i in 0..3 {
            let Some(b) = src.get(i) else {
                src.reserve(1);
                return Ok(None);
            };
            if s == 14 && *b > 0x03 {
                return Err(invalid_data(Overflow::<16>));
            }
            x |= (u16::from(*b) & 0x7f) << s;
            if b & 0x80 == 0 {
                return Ok(Some(x));
            }
            s += 7;
        }
        Err(invalid_data(Overflow::<16>))
    }
}

pub struct Leb128DecoderU32;

impl Decoder for Leb128DecoderU32 {
    type Item = u32;
    type Error = std::io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        let mut x = 0;
        let mut s = 0u8;
        for i in 0..5 {
            let Some(b) = src.get(i) else {
                src.reserve(1);
                return Ok(None);
            };
            if s == 28 && *b > 0x0f {
                return Err(invalid_data(Overflow::<32>));
            }
            x |= (u32::from(*b) & 0x7f) << s;
            if b & 0x80 == 0 {
                return Ok(Some(x));
            }
            s += 7;
        }
        Err(invalid_data(Overflow::<32>))
    }
}

pub struct Leb128DecoderU64;

impl Decoder for Leb128DecoderU64 {
    type Item = u64;
    type Error = std::io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        let mut x = 0;
        let mut s = 0u8;
        for i in 0..10 {
            let Some(b) = src.get(i) else {
                src.reserve(1);
                return Ok(None);
            };
            if s == 63 && *b > 0x01 {
                return Err(invalid_data(Overflow::<64>));
            }
            x |= (u64::from(*b) & 0x7f) << s;
            if b & 0x80 == 0 {
                return Ok(Some(x));
            }
            s += 7;
        }
        Err(invalid_data(Overflow::<64>))
    }
}

pub struct Leb128DecoderU128;

impl Decoder for Leb128DecoderU128 {
    type Item = u128;
    type Error = std::io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        let mut x = 0;
        let mut s = 0u8;
        for i in 0..19 {
            let Some(b) = src.get(i) else {
                src.reserve(1);
                return Ok(None);
            };
            if s == 126 && *b > 0x03 {
                return Err(invalid_data(Overflow::<128>));
            }
            x |= (u128::from(*b) & 0x7f) << s;
            if b & 0x80 == 0 {
                return Ok(Some(x));
            }
            s += 7;
        }
        Err(invalid_data(Overflow::<128>))
    }
}

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

impl Encoder<i8> for Leb128Encoder {
    type Error = std::io::Error;

    fn encode(&mut self, x: i8, dst: &mut BytesMut) -> Result<(), Self::Error> {
        dst.extend_from_slice(put_i8_leb128(&mut Default::default(), x));
        Ok(())
    }
}

impl Encoder<i16> for Leb128Encoder {
    type Error = std::io::Error;

    fn encode(&mut self, x: i16, dst: &mut BytesMut) -> Result<(), Self::Error> {
        dst.extend_from_slice(put_i16_leb128(&mut Default::default(), x));
        Ok(())
    }
}

impl Encoder<i32> for Leb128Encoder {
    type Error = std::io::Error;

    fn encode(&mut self, x: i32, dst: &mut BytesMut) -> Result<(), Self::Error> {
        dst.extend_from_slice(put_i32_leb128(&mut Default::default(), x));
        Ok(())
    }
}

impl Encoder<i64> for Leb128Encoder {
    type Error = std::io::Error;

    fn encode(&mut self, x: i64, dst: &mut BytesMut) -> Result<(), Self::Error> {
        dst.extend_from_slice(put_i64_leb128(&mut Default::default(), x));
        Ok(())
    }
}

impl Encoder<i128> for Leb128Encoder {
    type Error = std::io::Error;

    fn encode(&mut self, x: i128, dst: &mut BytesMut) -> Result<(), Self::Error> {
        dst.extend_from_slice(put_i128_leb128(&mut Default::default(), x));
        Ok(())
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

        let v = Leb128DecoderU32
            .decode(&mut ENCODED.as_slice().into())
            .expect("failed to decode u32");
        assert_eq!(v, Some(624_485));

        let v = ENCODED
            .as_slice()
            .read_u64_leb128()
            .await
            .expect("failed to read u64");
        assert_eq!(v, 624_485);

        let v = Leb128DecoderU64
            .decode(&mut ENCODED.as_slice().into())
            .expect("failed to decode u64");
        assert_eq!(v, Some(624_485));

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
    async fn signed_leb128() {
        const ENCODED: [u8; 3] = [0xc0, 0xbb, 0x78];

        let v = ENCODED
            .as_slice()
            .read_i32_leb128()
            .await
            .expect("failed to read i32");
        assert_eq!(v, -123_456);

        let v = ENCODED
            .as_slice()
            .read_i64_leb128()
            .await
            .expect("failed to read i64");
        assert_eq!(v, -123_456);

        let mut buf = vec![];
        buf.write_i32_leb128(-123_456)
            .await
            .expect("failed to write i32");
        assert_eq!(buf, ENCODED);

        let mut buf = vec![];
        buf.write_i64_leb128(-123_456)
            .await
            .expect("failed to write i64");
        assert_eq!(buf, ENCODED);

        let mut buf = vec![];
        buf.write_i64_leb128(-1).await.expect("failed to write i64");
        assert_eq!(buf, [0x7f]);

        let mut buf = vec![];
        buf.write_i64_leb128(-2).await.expect("failed to write i64");
        assert_eq!(buf, [0x7e]);

        let v = [0xff, 0x01]
            .as_slice()
            .read_i8_leb128()
            .await
            .expect("failed to read i8");
        assert_eq!(v, -1);

        let v = [0x7f]
            .as_slice()
            .read_i8_leb128()
            .await
            .expect("failed to read i8");
        assert_eq!(v, -1);

        let v = [0xff, 0x00]
            .as_slice()
            .read_i8_leb128()
            .await
            .expect("failed to read i8");
        assert_eq!(v, 0x7f);

        [0xff, 0x02]
            .as_slice()
            .read_i8_leb128()
            .await
            .expect_err("i8 read should have failed, since it encoded 9 bits");

        let v = [0xff, 0xff, 0x01]
            .as_slice()
            .read_i16_leb128()
            .await
            .expect("failed to read i16");
        assert_eq!(v, 0x7fff);

        let v = [0xff, 0xff, 0x02]
            .as_slice()
            .read_i16_leb128()
            .await
            .expect("failed to read i16");
        assert_eq!(v, -0x4001);

        let v = [0xff, 0xff, 0x03]
            .as_slice()
            .read_i16_leb128()
            .await
            .expect("failed to read i16");
        assert_eq!(v, -1);

        [0xff, 0xff, 0x04]
            .as_slice()
            .read_i16_leb128()
            .await
            .expect_err("i16 read should have failed, since it encoded 17 bits");

        let v = [0x7f]
            .as_slice()
            .read_i32_leb128()
            .await
            .expect("failed to read i32");
        assert_eq!(v, -1);

        let v = [0x7e]
            .as_slice()
            .read_i32_leb128()
            .await
            .expect("failed to read i32");
        assert_eq!(v, -2);

        let v = [0xff, 0xff, 0xff, 0xff, 0x0f]
            .as_slice()
            .read_i32_leb128()
            .await
            .expect("failed to read i32");
        assert_eq!(v, -1);

        let v = [0xfe, 0xff, 0xff, 0xff, 0x0f]
            .as_slice()
            .read_i32_leb128()
            .await
            .expect("failed to read i32");
        assert_eq!(v, -2);

        [0xff, 0xff, 0xff, 0xff, 0x10]
            .as_slice()
            .read_i32_leb128()
            .await
            .expect_err("i32 read should have failed, since it encoded 33 bits");

        let v = [0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x01]
            .as_slice()
            .read_i64_leb128()
            .await
            .expect("failed to read i64");
        assert_eq!(v, -1);

        [0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x02]
            .as_slice()
            .read_i64_leb128()
            .await
            .expect_err("i64 read should have failed, since it encoded 65 bits");

        let v = [
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff, 0xff, 0xff, 0xff, 0x03,
        ]
        .as_slice()
        .read_i128_leb128()
        .await
        .expect("failed to read i128");
        assert_eq!(v, -1);

        [
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff, 0xff, 0xff, 0xff, 0x04,
        ]
        .as_slice()
        .read_i128_leb128()
        .await
        .expect_err("i128 read should have failed, since it encoded 129 bits");
    }

    #[tokio::test]
    async fn unsigned() {
        let v = [0x01u8]
            .as_slice()
            .read_var_u8_leb128(2)
            .await
            .expect("failed to read u2");
        assert_eq!(v, 1);

        let v = [0x02]
            .as_slice()
            .read_var_u8_leb128(2)
            .await
            .expect("failed to read u2");
        assert_eq!(v, 2);

        [0b100]
            .as_slice()
            .read_var_u8_leb128(2)
            .await
            .expect_err("u2 read should have failed, since it encoded 3 bits");

        [0x80, 0x80, 0x01]
            .as_slice()
            .read_var_u16_leb128(9)
            .await
            .expect_err("u9 read should have failed, since it used over 9 bits");

        let v = [0x80, 0x80, 0x80, 0x80, 0x80, 0x01]
            .as_slice()
            .read_var_u64_leb128(64)
            .await
            .expect("failed to read u64");
        assert_eq!(v, 0b1000_0000_0000_0000_0000_0000_0000_0000_0000);
    }
}
