#![allow(clippy::cast_possible_truncation)]

use ::core::fmt::Display;
use ::core::future::Future;

use tokio::io::{AsyncRead, AsyncReadExt as _, AsyncWrite, AsyncWriteExt as _};
use tokio_util::bytes::{Buf as _, BytesMut};
use tokio_util::codec::{Decoder, Encoder};

/// Error returned for overflows decoding statically-sized integers
#[derive(Debug)]
pub struct Overflow<const N: usize>;

impl<const N: usize> Display for Overflow<N> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "varint overflows {N}-bit integer")
    }
}

impl<const N: usize> std::error::Error for Overflow<N> {}

/// Error returned for overflows decoding variable size integers
#[derive(Debug)]
pub struct OverflowVar(u8);

impl Display for OverflowVar {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "varint overflows {}-bit integer", self.0)
    }
}

impl std::error::Error for OverflowVar {}

fn invalid_data(err: impl Sync + Send + std::error::Error + 'static) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, err)
}

fn is_sign_extended(b: u8, bits: u32) -> bool {
    let shift = bits.saturating_sub(1);
    let rest = (b & 0x7f) >> shift;
    rest == 0 || rest == 0x7f >> shift
}

macro_rules! decode_unsigned_leb128 {
    ($t:ident, $i:ident => $next:expr, $x:ident => $ret:expr) => {
        decode_unsigned_leb128!(
            $t,
            <$t>::BITS,
            Overflow::<{ <$t>::BITS as usize }>,
            $i => $next,
            $x => $ret
        )
    };
    ($t:ident, $bits:expr, $err:expr, $i:ident => $next:expr, $x:ident => $ret:expr) => {{
        let bits: u32 = $bits;
        let last = bits.saturating_sub(1) / 7 * 7;
        let max: u8 = (1 << bits.saturating_sub(last)) - 1;
        let mut $x: $t = 0;
        for ($i, s) in (0..=last).step_by(7).enumerate() {
            let b: u8 = $next;
            if s == last && b > max {
                break;
            }
            $x |= (<$t>::from(b) & 0x7f) << s;
            if b & 0x80 == 0 {
                return $ret;
            }
        }
        Err(invalid_data($err))
    }};
}

macro_rules! decode_signed_leb128 {
    ($t:ident, $i:ident => $next:expr, $x:ident => $ret:expr) => {{
        const LAST: u32 = (<$t>::BITS - 1) / 7 * 7;
        let mut $x: $t = 0;
        for ($i, s) in (0..=LAST).step_by(7).enumerate() {
            let b: u8 = $next;
            if s == LAST && !is_sign_extended(b, <$t>::BITS - LAST) {
                break;
            }
            $x |= <$t>::from((b & 0x7f).cast_signed()) << s;
            if b & 0x80 == 0 {
                if s != LAST && b & 0x40 != 0 {
                    $x |= !0 << (s + 7);
                }
                return $ret;
            }
        }
        Err(invalid_data(Overflow::<{ <$t>::BITS as usize }>))
    }};
}

macro_rules! read_leb128 {
    ($f:ident, $t:ident, $decode:ident) => {
        #[cfg_attr(
            feature = "tracing",
            tracing::instrument(level = "trace", ret, skip_all, fields(ty = stringify!($t)))
        )]
        fn $f(&mut self) -> impl Future<Output = std::io::Result<$t>>
        where
            Self: Unpin,
        {
            async move { $decode!($t, _i => self.read_u8().await?, x => Ok(x)) }
        }
    };
}

macro_rules! read_var_unsigned_leb128 {
    ($f:ident, $t:ident, $bits:literal, $fixed:ident, [$($sbits:literal => $sfixed:ident),*]) => {
        #[cfg_attr(
            feature = "tracing",
            tracing::instrument(level = "trace", ret, skip_all, fields(ty = "uvarint", n = n))
        )]
        fn $f(&mut self, n: u8) -> impl Future<Output = std::io::Result<$t>>
        where
            Self: Unpin,
        {
            async move {
                match n {
                    $($sbits => return self.$sfixed().await.map(Into::into),)*
                    $bits => return self.$fixed().await,
                    n if n > $bits => {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::InvalidInput,
                            format!("{n}-bit integer does not fit in `{}`", stringify!($t)),
                        ))
                    }
                    _ => {}
                }
                decode_unsigned_leb128!(
                    $t,
                    u32::from(n),
                    OverflowVar(n),
                    _i => self.read_u8().await?,
                    x => Ok(x)
                )
            }
        }
    };
}

pub trait AsyncReadLeb128: AsyncRead {
    read_leb128!(read_u8_leb128, u8, decode_unsigned_leb128);
    read_leb128!(read_u16_leb128, u16, decode_unsigned_leb128);
    read_leb128!(read_u32_leb128, u32, decode_unsigned_leb128);
    read_leb128!(read_u64_leb128, u64, decode_unsigned_leb128);
    read_leb128!(read_u128_leb128, u128, decode_unsigned_leb128);

    read_var_unsigned_leb128!(read_var_u8_leb128, u8, 8, read_u8_leb128, []);
    read_var_unsigned_leb128!(
        read_var_u16_leb128,
        u16,
        16,
        read_u16_leb128,
        [8 => read_u8_leb128]
    );
    read_var_unsigned_leb128!(
        read_var_u32_leb128,
        u32,
        32,
        read_u32_leb128,
        [8 => read_u8_leb128, 16 => read_u16_leb128]
    );
    read_var_unsigned_leb128!(
        read_var_u64_leb128,
        u64,
        64,
        read_u64_leb128,
        [8 => read_u8_leb128, 16 => read_u16_leb128, 32 => read_u32_leb128]
    );
    read_var_unsigned_leb128!(
        read_var_u128_leb128,
        u128,
        128,
        read_u128_leb128,
        [8 => read_u8_leb128, 16 => read_u16_leb128, 32 => read_u32_leb128, 64 => read_u64_leb128]
    );

    read_leb128!(read_i8_leb128, i8, decode_signed_leb128);
    read_leb128!(read_i16_leb128, i16, decode_signed_leb128);
    read_leb128!(read_i32_leb128, i32, decode_signed_leb128);
    read_leb128!(read_i64_leb128, i64, decode_signed_leb128);
    read_leb128!(read_i128_leb128, i128, decode_signed_leb128);
}

impl<T: AsyncRead> AsyncReadLeb128 for T {}

macro_rules! write_leb128 {
    ($f:ident, $t:ident, $put:ident) => {
        #[cfg_attr(
                    feature = "tracing",
                    tracing::instrument(level = "trace", ret, skip_all, fields(ty = stringify!($t)))
                )]
        fn $f(&mut self, x: $t) -> impl Future<Output = std::io::Result<()>>
        where
            Self: Unpin,
        {
            async move { self.write_all($put(&mut Default::default(), x)).await }
        }
    };
}

pub trait AsyncWriteLeb128: AsyncWrite {
    write_leb128!(write_u8_leb128, u8, put_u8_leb128);
    write_leb128!(write_u16_leb128, u16, put_u16_leb128);
    write_leb128!(write_u32_leb128, u32, put_u32_leb128);
    write_leb128!(write_u64_leb128, u64, put_u64_leb128);
    write_leb128!(write_u128_leb128, u128, put_u128_leb128);
    write_leb128!(write_i8_leb128, i8, put_i8_leb128);
    write_leb128!(write_i16_leb128, i16, put_i16_leb128);
    write_leb128!(write_i32_leb128, i32, put_i32_leb128);
    write_leb128!(write_i64_leb128, i64, put_i64_leb128);
    write_leb128!(write_i128_leb128, i128, put_i128_leb128);
}

impl<T: AsyncWrite> AsyncWriteLeb128 for T {}

fn stateless_decode_eof<D>(dec: &mut D, buf: &mut BytesMut) -> Result<Option<D::Item>, D::Error>
where
    D: Decoder,
{
    match dec.decode(buf)? {
        Some(frame) => Ok(Some(frame)),
        None => {
            if buf.is_empty() {
                Ok(None)
            } else {
                Err(std::io::Error::from(std::io::ErrorKind::UnexpectedEof).into())
            }
        }
    }
}

pub struct Leb128Encoder;

macro_rules! impl_encode {
    ($t:ty, $f:ident) => {
        impl Encoder<$t> for Leb128Encoder {
            type Error = std::io::Error;

            fn encode(&mut self, item: $t, dst: &mut BytesMut) -> Result<(), Self::Error> {
                dst.extend_from_slice($f(&mut Default::default(), item));
                Ok(())
            }
        }

        impl Encoder<&$t> for Leb128Encoder {
            type Error = std::io::Error;

            fn encode(&mut self, item: &$t, dst: &mut BytesMut) -> Result<(), Self::Error> {
                self.encode(*item, dst)
            }
        }

        impl Encoder<&&$t> for Leb128Encoder {
            type Error = std::io::Error;

            fn encode(&mut self, item: &&$t, dst: &mut BytesMut) -> Result<(), Self::Error> {
                self.encode(*item, dst)
            }
        }
    };
}

macro_rules! leb128_decoder {
    ($t:ident, $decoder:ident, $decode:ident) => {
        #[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
        pub struct $decoder;

        impl Decoder for $decoder {
            type Item = $t;
            type Error = std::io::Error;

            fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
                $decode!(
                    $t,
                    i => {
                        let Some(b) = src.get(i).copied() else {
                            src.reserve(1);
                            return Ok(None);
                        };
                        b
                    },
                    x => {
                        src.advance(i + 1);
                        Ok(Some(x))
                    }
                )
            }

            fn decode_eof(
                &mut self,
                src: &mut BytesMut,
            ) -> Result<Option<Self::Item>, Self::Error> {
                stateless_decode_eof(self, src)
            }
        }
    };
}

macro_rules! unsigned_leb128 {
    ($t:ident, $put:ident, $decoder:ident) => {
        pub fn $put(buf: &mut [u8; <$t>::BITS.div_ceil(7) as usize], mut x: $t) -> &mut [u8] {
            let mut i = 0;
            while x >= 0x80 {
                buf[i] = (x as u8) | 0x80;
                x >>= 7;
                i += 1;
            }
            buf[i] = x as u8;
            &mut buf[..=i]
        }

        leb128_decoder!($t, $decoder, decode_unsigned_leb128);
        impl_encode!($t, $put);
    };
}

macro_rules! signed_leb128 {
    ($t:ident, $put:ident, $decoder:ident) => {
        pub fn $put(buf: &mut [u8; <$t>::BITS.div_ceil(7) as usize], mut x: $t) -> &mut [u8] {
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

        leb128_decoder!($t, $decoder, decode_signed_leb128);
        impl_encode!($t, $put);
    };
}

unsigned_leb128!(u8, put_u8_leb128, Leb128DecoderU8);
unsigned_leb128!(u16, put_u16_leb128, Leb128DecoderU16);
unsigned_leb128!(u32, put_u32_leb128, Leb128DecoderU32);
unsigned_leb128!(u64, put_u64_leb128, Leb128DecoderU64);
unsigned_leb128!(u128, put_u128_leb128, Leb128DecoderU128);
signed_leb128!(i8, put_i8_leb128, Leb128DecoderI8);
signed_leb128!(i16, put_i16_leb128, Leb128DecoderI16);
signed_leb128!(i32, put_i32_leb128, Leb128DecoderI32);
signed_leb128!(i64, put_i64_leb128, Leb128DecoderI64);
signed_leb128!(i128, put_i128_leb128, Leb128DecoderI128);

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

        let v = [0x80, 0x7f]
            .as_slice()
            .read_i8_leb128()
            .await
            .expect("failed to read i8");
        assert_eq!(v, i8::MIN);

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

        let v = [0xff, 0xff, 0x7e]
            .as_slice()
            .read_i16_leb128()
            .await
            .expect("failed to read i16");
        assert_eq!(v, -0x4001);

        let v = [0xff, 0xff, 0x7f]
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

        let v = [0xff, 0xff, 0xff, 0xff, 0x7f]
            .as_slice()
            .read_i32_leb128()
            .await
            .expect("failed to read i32");
        assert_eq!(v, -1);

        let v = [0xfe, 0xff, 0xff, 0xff, 0x7f]
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

        let v = [0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x7f]
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
            0xff, 0xff, 0xff, 0xff, 0x7f,
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
    async fn signed_leb128_roundtrip() {
        macro_rules! roundtrip {
            ($write:ident, $read:ident, $decoder:ident, $values:expr) => {
                for v in $values {
                    let mut buf = vec![];
                    buf.$write(v).await.expect("failed to encode");

                    let got = buf
                        .as_slice()
                        .$read()
                        .await
                        .expect("failed to read back encoded value");
                    assert_eq!(got, v, "async read mismatch for {v}, encoded as {buf:02x?}");

                    let got = $decoder
                        .decode(&mut buf.as_slice().into())
                        .expect("failed to decode back encoded value")
                        .expect("decoder returned no value");
                    assert_eq!(got, v, "Decoder mismatch for {v}, encoded as {buf:02x?}");
                }
            };
        }

        roundtrip!(
            write_i8_leb128,
            read_i8_leb128,
            Leb128DecoderI8,
            [i8::MIN, i8::MAX, -1]
        );
        roundtrip!(
            write_i16_leb128,
            read_i16_leb128,
            Leb128DecoderI16,
            [i16::MIN, i16::MAX, -1]
        );
        roundtrip!(
            write_i32_leb128,
            read_i32_leb128,
            Leb128DecoderI32,
            [i32::MIN, i32::MAX, -1]
        );
        roundtrip!(
            write_i64_leb128,
            read_i64_leb128,
            Leb128DecoderI64,
            [i64::MIN, i64::MAX, -1]
        );
        roundtrip!(
            write_i128_leb128,
            read_i128_leb128,
            Leb128DecoderI128,
            [i128::MIN, i128::MAX, -1]
        );
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

        let v = [0x03]
            .as_slice()
            .read_var_u8_leb128(2)
            .await
            .expect("failed to read u2");
        assert_eq!(v, 3);

        let v = [0xff, 0x03]
            .as_slice()
            .read_var_u16_leb128(9)
            .await
            .expect("failed to read u9");
        assert_eq!(v, 0x1ff);

        [0xff, 0x04]
            .as_slice()
            .read_var_u16_leb128(9)
            .await
            .expect_err("u9 read should have failed, since it encoded 10 bits");

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

        let v = [0x7f]
            .as_slice()
            .read_var_u8_leb128(7)
            .await
            .expect("failed to read u7");
        assert_eq!(v, 0x7f);

        [0x85, 0x00]
            .as_slice()
            .read_var_u8_leb128(7)
            .await
            .expect_err("u7 read should have failed, since it used over 1 byte");

        [0x80, 0x80, 0x00]
            .as_slice()
            .read_var_u16_leb128(14)
            .await
            .expect_err("u14 read should have failed, since it used over 2 bytes");

        let err = [0x01]
            .as_slice()
            .read_var_u8_leb128(20)
            .await
            .expect_err("u20 read into u8 should have failed");
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);

        let v = [0x80, 0x80, 0x80, 0x80, 0x80, 0x01]
            .as_slice()
            .read_var_u64_leb128(64)
            .await
            .expect("failed to read u64");
        assert_eq!(v, 0b1000_0000_0000_0000_0000_0000_0000_0000_0000);
    }

    #[test]
    fn truncated_eof() {
        let mut src = BytesMut::from(b"\xe5\x8e".as_slice());
        let err = Leb128DecoderU32
            .decode_eof(&mut src)
            .expect_err("truncated u32 must fail");
        assert_eq!(err.kind(), std::io::ErrorKind::UnexpectedEof);

        let mut src = BytesMut::from(b"\x80".as_slice());
        let err = Leb128DecoderI64
            .decode_eof(&mut src)
            .expect_err("truncated i64 must fail");
        assert_eq!(err.kind(), std::io::ErrorKind::UnexpectedEof);

        assert!(Leb128DecoderU32
            .decode_eof(&mut BytesMut::new())
            .unwrap()
            .is_none());
    }
}
