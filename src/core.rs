use ::core::future::Future;
use ::core::mem;
use ::core::str;

use std::sync::Arc;

use leb128_tokio::{AsyncReadLeb128, Leb128DecoderU32, Leb128Encoder};
use tokio::io::{AsyncRead, AsyncReadExt as _, AsyncWrite, AsyncWriteExt as _};
use tokio_util::bytes::{BufMut as _, Bytes, BytesMut};
use tokio_util::codec::{Decoder, Encoder};

pub trait AsyncReadCore: AsyncRead {
    /// Read [`core:name`](https://webassembly.github.io/spec/core/binary/values.html#names)
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(level = "trace", ret, skip_all, fields(ty = "name"))
    )]
    fn read_core_name(&mut self, s: &mut String) -> impl Future<Output = std::io::Result<()>>
    where
        Self: Unpin + Sized,
    {
        async move {
            let n = self.read_u32_leb128().await?;
            let len = usize::try_from(n).unwrap_or(usize::MAX);
            let mut buf = Vec::with_capacity(len.min(DEFAULT_MAX_INITIAL_CAPACITY));
            if self.take(n.into()).read_to_end(&mut buf).await? != len {
                return Err(std::io::ErrorKind::UnexpectedEof.into());
            }
            let buf = String::from_utf8(buf)
                .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))?;
            s.push_str(&buf);
            Ok(())
        }
    }
}

impl<T: AsyncRead> AsyncReadCore for T {}

pub trait AsyncWriteCore: AsyncWrite {
    /// Write [`core:name`](https://webassembly.github.io/spec/core/binary/values.html#names)
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(level = "trace", ret, skip_all, fields(ty = "name"))
    )]
    fn write_core_name(&mut self, s: &str) -> impl Future<Output = std::io::Result<()>>
    where
        Self: Unpin,
    {
        async move {
            let mut buf = BytesMut::with_capacity(5usize.saturating_add(s.len()));
            CoreNameEncoder.encode(s, &mut buf)?;
            self.write_all(&buf).await
        }
    }
}

impl<T: AsyncWrite> AsyncWriteCore for T {}

/// [`core:name`](https://webassembly.github.io/spec/core/binary/values.html#names) encoder
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct CoreNameEncoder;

impl<T: AsRef<str>> Encoder<T> for CoreNameEncoder {
    type Error = std::io::Error;

    fn encode(&mut self, item: T, dst: &mut BytesMut) -> Result<(), Self::Error> {
        let item = item.as_ref();
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

/// [`core:name`](https://webassembly.github.io/spec/core/binary/values.html#names) decoder
///
/// At most `MAX_INITIAL_CAPACITY` bytes are preallocated based on the name length read from the input
#[derive(Debug)]
pub struct CoreNameDecoder<const MAX_INITIAL_CAPACITY: usize = DEFAULT_MAX_INITIAL_CAPACITY>(
    CoreVecDecoderBytes<MAX_INITIAL_CAPACITY>,
);

impl Default for CoreNameDecoder {
    fn default() -> Self {
        Self::capped()
    }
}

impl<const MAX_INITIAL_CAPACITY: usize> CoreNameDecoder<MAX_INITIAL_CAPACITY> {
    pub fn capped() -> Self {
        Self(CoreVecDecoderBytes::capped())
    }
}

impl<const MAX_INITIAL_CAPACITY: usize> Decoder for CoreNameDecoder<MAX_INITIAL_CAPACITY> {
    type Item = String;
    type Error = std::io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        let Some(buf) = self.0.decode(src)? else {
            return Ok(None);
        };
        let s = str::from_utf8(&buf)
            .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))?;
        Ok(Some(s.to_string()))
    }
}

/// [`core:vec`](https://webassembly.github.io/spec/core/binary/conventions.html#binary-vec) encoder
pub struct CoreVecEncoder<E>(pub E);

impl<E, T, const N: usize> Encoder<[T; N]> for CoreVecEncoder<E>
where
    E: Encoder<T>,
    std::io::Error: From<E::Error>,
{
    type Error = std::io::Error;

    fn encode(&mut self, item: [T; N], dst: &mut BytesMut) -> Result<(), Self::Error> {
        dst.reserve(5 + N);
        let len = u32::try_from(N)
            .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidInput, err))?;
        Leb128Encoder.encode(len, dst)?;
        for item in item {
            self.0.encode(item, dst)?;
        }
        Ok(())
    }
}

impl<E, T> Encoder<Vec<T>> for CoreVecEncoder<E>
where
    E: Encoder<T>,
    std::io::Error: From<E::Error>,
{
    type Error = std::io::Error;

    fn encode(&mut self, item: Vec<T>, dst: &mut BytesMut) -> Result<(), Self::Error> {
        let len = item.len();
        dst.reserve(5 + len);
        let len = u32::try_from(len)
            .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidInput, err))?;
        Leb128Encoder.encode(len, dst)?;
        for item in item {
            self.0.encode(item, dst)?;
        }
        Ok(())
    }
}

impl<E, T> Encoder<Box<[T]>> for CoreVecEncoder<E>
where
    E: Encoder<T>,
    std::io::Error: From<E::Error>,
{
    type Error = std::io::Error;

    fn encode(&mut self, item: Box<[T]>, dst: &mut BytesMut) -> Result<(), Self::Error> {
        self.encode(Vec::from(item), dst)
    }
}

impl<'a, E, T, const N: usize> Encoder<&'a [T; N]> for CoreVecEncoder<E>
where
    E: Encoder<&'a T>,
    std::io::Error: From<E::Error>,
{
    type Error = std::io::Error;

    fn encode(&mut self, item: &'a [T; N], dst: &mut BytesMut) -> Result<(), Self::Error> {
        dst.reserve(5 + N);
        let len = u32::try_from(N)
            .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidInput, err))?;
        Leb128Encoder.encode(len, dst)?;
        for item in item {
            self.0.encode(item, dst)?;
        }
        Ok(())
    }
}

impl<'a, E, T> Encoder<&'a [T]> for CoreVecEncoder<E>
where
    E: Encoder<&'a T>,
    std::io::Error: From<E::Error>,
{
    type Error = std::io::Error;

    fn encode(&mut self, item: &'a [T], dst: &mut BytesMut) -> Result<(), Self::Error> {
        let len = item.len();
        dst.reserve(5 + len);
        let len = u32::try_from(len)
            .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidInput, err))?;
        Leb128Encoder.encode(len, dst)?;
        for item in item {
            self.0.encode(item, dst)?;
        }
        Ok(())
    }
}

impl<'a, 'b, E, T> Encoder<&'a &'b [T]> for CoreVecEncoder<E>
where
    E: Encoder<&'b T>,
    std::io::Error: From<E::Error>,
{
    type Error = std::io::Error;

    fn encode(&mut self, item: &'a &'b [T], dst: &mut BytesMut) -> Result<(), Self::Error> {
        self.encode(*item, dst)
    }
}

impl<'a, E, T> Encoder<&'a Vec<T>> for CoreVecEncoder<E>
where
    E: Encoder<&'a T>,
    std::io::Error: From<E::Error>,
{
    type Error = std::io::Error;

    fn encode(&mut self, item: &'a Vec<T>, dst: &mut BytesMut) -> Result<(), Self::Error> {
        self.encode(item.as_slice(), dst)
    }
}

impl<'a, 'b, E, T> Encoder<&'a &'b Vec<T>> for CoreVecEncoder<E>
where
    E: Encoder<&'b T>,
    std::io::Error: From<E::Error>,
{
    type Error = std::io::Error;

    fn encode(&mut self, item: &'a &'b Vec<T>, dst: &mut BytesMut) -> Result<(), Self::Error> {
        self.encode(item.as_slice(), dst)
    }
}

impl<'a, E, T> Encoder<&'a Box<[T]>> for CoreVecEncoder<E>
where
    E: Encoder<&'a T>,
    std::io::Error: From<E::Error>,
{
    type Error = std::io::Error;

    fn encode(&mut self, item: &'a Box<[T]>, dst: &mut BytesMut) -> Result<(), Self::Error> {
        let item: &[T] = item.as_ref();
        self.encode(item, dst)
    }
}

impl<E, T> Encoder<Arc<[T]>> for CoreVecEncoder<E>
where
    for<'a> E: Encoder<&'a T>,
    for<'a> std::io::Error: From<<E as Encoder<&'a T>>::Error>,
{
    type Error = std::io::Error;

    fn encode(&mut self, item: Arc<[T]>, dst: &mut BytesMut) -> Result<(), Self::Error> {
        let item = item.as_ref();
        let len = item.len();
        dst.reserve(5 + len);
        let len = u32::try_from(len)
            .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidInput, err))?;
        Leb128Encoder.encode(len, dst)?;
        for item in item {
            self.0.encode(item, dst)?;
        }
        Ok(())
    }
}

impl<'a, E, T> Encoder<&'a Arc<[T]>> for CoreVecEncoder<E>
where
    E: Encoder<&'a T>,
    std::io::Error: From<E::Error>,
{
    type Error = std::io::Error;

    fn encode(&mut self, item: &'a Arc<[T]>, dst: &mut BytesMut) -> Result<(), Self::Error> {
        let item: &[T] = item.as_ref();
        self.encode(item, dst)
    }
}

/// Default maximum number of bytes decoders preallocate based on a length read from the input
pub const DEFAULT_MAX_INITIAL_CAPACITY: usize = 1 << 20;

/// [`core:vec`](https://webassembly.github.io/spec/core/binary/conventions.html#binary-vec) decoder
///
/// At most `MAX_INITIAL_CAPACITY` bytes are preallocated based on the vector length read from the input
#[derive(Debug)]
pub struct CoreVecDecoder<
    T: Decoder,
    const MAX_INITIAL_CAPACITY: usize = DEFAULT_MAX_INITIAL_CAPACITY,
> {
    dec: T,
    ret: Vec<T::Item>,
    cap: usize,
}

impl<T> CoreVecDecoder<T>
where
    T: Decoder,
{
    pub fn new(decoder: T) -> Self {
        Self::capped(decoder)
    }
}

impl<T, const MAX_INITIAL_CAPACITY: usize> CoreVecDecoder<T, MAX_INITIAL_CAPACITY>
where
    T: Decoder,
{
    pub fn capped(decoder: T) -> Self {
        Self {
            dec: decoder,
            ret: Vec::default(),
            cap: 0,
        }
    }

    pub fn into_inner(self) -> T {
        self.dec
    }
}

impl<T> Default for CoreVecDecoder<T>
where
    T: Decoder + Default,
{
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<T, const MAX_INITIAL_CAPACITY: usize> Decoder for CoreVecDecoder<T, MAX_INITIAL_CAPACITY>
where
    T: Decoder,
{
    type Item = Vec<T::Item>;
    type Error = T::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if self.cap == 0 {
            let Some(len) = Leb128DecoderU32.decode(src)? else {
                return Ok(None);
            };
            if len == 0 {
                return Ok(Some(Vec::default()));
            }
            let len = len
                .try_into()
                .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidInput, err))?;
            self.cap = len;
            self.ret = Vec::with_capacity(
                len.min(MAX_INITIAL_CAPACITY / mem::size_of::<T::Item>().max(1)),
            );
        }
        while self.cap > 0 {
            let v = match self.dec.decode(src) {
                Ok(Some(v)) => v,
                Ok(None) => return Ok(None),
                Err(err) => {
                    self.cap = 0;
                    self.ret = Vec::default();
                    return Err(err);
                }
            };
            self.ret.push(v);
            self.cap -= 1;
        }
        Ok(Some(mem::take(&mut self.ret)))
    }
}

/// [`core:vec`](https://webassembly.github.io/spec/core/binary/conventions.html#binary-vec)
/// encoder optimized for vectors of byte-sized values
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct CoreVecEncoderBytes;

impl<T: AsRef<[u8]>> Encoder<T> for CoreVecEncoderBytes {
    type Error = std::io::Error;

    fn encode(&mut self, item: T, dst: &mut BytesMut) -> Result<(), Self::Error> {
        let item = item.as_ref();
        let n = item.len();
        let n = u32::try_from(n)
            .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidInput, err))?;
        dst.reserve(item.len().saturating_add(5));
        Leb128Encoder.encode(n, dst)?;
        dst.extend_from_slice(item);
        Ok(())
    }
}

/// [`core:vec`](https://webassembly.github.io/spec/core/binary/conventions.html#binary-vec)
/// decoder optimized for vectors of byte-sized values
///
/// At most `MAX_INITIAL_CAPACITY` bytes are preallocated based on the vector length read from the input
#[derive(Debug)]
pub struct CoreVecDecoderBytes<const MAX_INITIAL_CAPACITY: usize = DEFAULT_MAX_INITIAL_CAPACITY>(
    usize,
);

impl Default for CoreVecDecoderBytes {
    fn default() -> Self {
        Self::capped()
    }
}

impl<const MAX_INITIAL_CAPACITY: usize> CoreVecDecoderBytes<MAX_INITIAL_CAPACITY> {
    pub fn capped() -> Self {
        Self(0)
    }
}

impl<const MAX_INITIAL_CAPACITY: usize> Decoder for CoreVecDecoderBytes<MAX_INITIAL_CAPACITY> {
    type Item = Bytes;
    type Error = std::io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if self.0 == 0 {
            let Some(len) = Leb128DecoderU32.decode(src)? else {
                return Ok(None);
            };
            if len == 0 {
                return Ok(Some(Bytes::default()));
            }
            let len = len
                .try_into()
                .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidInput, err))?;
            self.0 = len;
        }
        if src.len() < self.0 {
            src.reserve(self.0.saturating_sub(src.len()).min(MAX_INITIAL_CAPACITY));
            return Ok(None);
        }
        let buf = src.split_to(self.0);
        self.0 = 0;
        Ok(Some(buf.freeze()))
    }
}

#[cfg(test)]
mod tests {
    use futures::{SinkExt as _, TryStreamExt as _};
    use tokio_util::codec::{FramedRead, FramedWrite};
    use tracing::trace;

    use super::*;

    #[test_log::test(tokio::test)]
    async fn string() {
        let mut s = String::new();
        "\x04test"
            .as_bytes()
            .read_core_name(&mut s)
            .await
            .expect("failed to read string");
        assert_eq!(s, "test");

        let mut buf = vec![];
        buf.write_core_name("test")
            .await
            .expect("failed to write string");
        assert_eq!(buf, b"\x04test");

        let mut tx = FramedWrite::new(Vec::new(), CoreNameEncoder);

        trace!("sending `foo`");
        tx.send("foo").await.expect("failed to send `foo`");

        trace!("sending ``");
        tx.send(String::default()).await.expect("failed to send ``");

        trace!("sending `test`");
        tx.send(&&&&&&"test").await.expect("failed to send `test`");

        trace!("sending `bar`");
        tx.send(Arc::from("bar"))
            .await
            .expect("failed to send `bar`");

        trace!("sending `ƒ𐍈Ő`");
        tx.send(&&String::from("ƒ𐍈Ő"))
            .await
            .expect("failed to send `ƒ𐍈Ő`");

        trace!("sending `baz`");
        tx.send(&&&Arc::from("baz"))
            .await
            .expect("failed to send `baz`");

        let tx = tx.into_inner();
        assert_eq!(
            tx,
            concat!("\x03foo", "\0", "\x04test", "\x03bar", "\x08ƒ𐍈Ő", "\x03baz").as_bytes()
        );
        let mut rx = FramedRead::new(tx.as_slice(), CoreNameDecoder::default());

        trace!("reading `foo`");
        let s = rx.try_next().await.expect("failed to get `foo`");
        assert_eq!(s.as_deref(), Some("foo"));

        trace!("reading ``");
        let s = rx.try_next().await.expect("failed to get ``");
        assert_eq!(s.as_deref(), Some(""));

        trace!("reading `test`");
        let s = rx.try_next().await.expect("failed to get `test`");
        assert_eq!(s.as_deref(), Some("test"));

        trace!("reading `bar`");
        let s = rx.try_next().await.expect("failed to get `bar`");
        assert_eq!(s.as_deref(), Some("bar"));

        trace!("reading `ƒ𐍈Ő`");
        let s = rx.try_next().await.expect("failed to get `ƒ𐍈Ő`");
        assert_eq!(s.as_deref(), Some("ƒ𐍈Ő"));

        trace!("reading `baz`");
        let s = rx.try_next().await.expect("failed to get `baz`");
        assert_eq!(s.as_deref(), Some("baz"));

        let s = rx.try_next().await.expect("failed to get EOF");
        assert_eq!(s, None);
    }

    #[test_log::test(tokio::test)]
    async fn vec() {
        let mut tx = FramedWrite::new(Vec::new(), CoreVecEncoder(CoreNameEncoder));

        trace!("sending [`foo`, ``, `test`, `bar`, `ƒ𐍈Ő`, `baz`]");
        tx.send(&["foo", "", "test", "bar", "ƒ𐍈Ő", "baz"])
            .await
            .expect("failed to send [`foo`, ``, `test`, `bar`, `ƒ𐍈Ő`, `baz`]");

        trace!("sending [``]");
        tx.send(&[""; 0]).await.expect("failed to send []");

        trace!("sending [`test`]");
        tx.send(&["test"]).await.expect("failed to send [`test`]");

        trace!("sending [``]");
        tx.send(&[""; 0]).await.expect("failed to send []");

        let tx = tx.into_inner();
        assert_eq!(
            tx,
            concat!(
                concat!(
                    "\x06",
                    concat!("\x03foo", "\0", "\x04test", "\x03bar", "\x08ƒ𐍈Ő", "\x03baz")
                ),
                "\0",
                concat!("\x01", "\x04test"),
                "\0"
            )
            .as_bytes()
        );
        let mut rx = FramedRead::new(tx.as_slice(), CoreVecDecoder::<CoreNameDecoder>::default());

        trace!("reading [`foo`, ``, `test`, `bar`, `baz`]");
        let s = rx
            .try_next()
            .await
            .expect("failed to get [`foo`, ``, `test`, `bar`, `baz`]");
        assert_eq!(
            s.as_deref(),
            Some(
                [
                    "foo".to_string(),
                    String::new(),
                    "test".to_string(),
                    "bar".to_string(),
                    "ƒ𐍈Ő".to_string(),
                    "baz".to_string()
                ]
                .as_slice()
            )
        );

        trace!("reading []");
        let s = rx.try_next().await.expect("failed to get []");
        assert_eq!(s.as_deref(), Some([].as_slice()));

        trace!("reading [`test`]");
        let s = rx.try_next().await.expect("failed to get [`test`]");
        assert_eq!(s.as_deref(), Some(["test".to_string()].as_slice()));

        trace!("reading []");
        let s = rx.try_next().await.expect("failed to get []");
        assert_eq!(s.as_deref(), Some([].as_slice()));

        let s = rx.try_next().await.expect("failed to get EOF");
        assert_eq!(s, None);
    }

    #[test_log::test(tokio::test)]
    async fn string_truncated() {
        let mut s = String::default();
        let err = b"\xff\xff\xff\xff\x0ftest"
            .as_slice()
            .read_core_name(&mut s)
            .await
            .expect_err("truncated string must fail");
        assert_eq!(err.kind(), std::io::ErrorKind::UnexpectedEof);

        let mut s = String::from("x");
        let err = b"\x0aabc"
            .as_slice()
            .read_core_name(&mut s)
            .await
            .expect_err("truncated string must fail");
        assert_eq!(err.kind(), std::io::ErrorKind::UnexpectedEof);
        assert_eq!(s, "x");

        let err = b"\x04\xe2\x82"
            .as_slice()
            .read_core_name(&mut s)
            .await
            .expect_err("truncated string must fail");
        assert_eq!(err.kind(), std::io::ErrorKind::UnexpectedEof);
    }

    #[test_log::test(tokio::test)]
    async fn vec_capped() {
        let mut rx = FramedRead::new(
            b"\x03\x01a\x01b\x01c".as_slice(),
            CoreVecDecoder::<_, 1>::capped(CoreNameDecoder::<1>::capped()),
        );
        let vs = rx.try_next().await.unwrap().unwrap();
        assert_eq!(vs, ["a", "b", "c"]);
        let mut rx = FramedRead::new(b"\x04test".as_slice(), CoreVecDecoderBytes::<1>::capped());
        let buf = rx.try_next().await.unwrap().unwrap();
        assert_eq!(buf, b"test".as_slice());

        let mut dec = CoreVecDecoder::<_, 64>::capped(CoreNameDecoder::<64>::capped());
        let mut src = BytesMut::from(b"\xff\xff\xff\xff\x0f\xff\xff\xff\xff\x0f".as_slice());
        assert!(dec.decode(&mut src).unwrap().is_none());
        assert!(dec.ret.capacity() <= 64 / mem::size_of::<String>());
        assert!(src.capacity() <= 128);
    }

    #[test_log::test(tokio::test)]
    async fn vec_reset_on_error() {
        let mut dec = CoreVecDecoder::<CoreNameDecoder>::default();
        let mut src = BytesMut::from(b"\x03\x01a\x01\xff".as_slice());
        dec.decode(&mut src).expect_err("invalid UTF-8 must fail");
        let mut src = BytesMut::from(b"\x01\x01b".as_slice());
        assert_eq!(dec.decode(&mut src).unwrap().unwrap(), ["b"]);
    }

    #[test_log::test(tokio::test)]
    async fn inference() {
        let _ = FramedRead::new(b"".as_slice(), CoreVecDecoderBytes::default());
        let _ = FramedRead::new(b"".as_slice(), CoreNameDecoder::default());
        let _ = FramedRead::new(
            b"".as_slice(),
            CoreVecDecoder::new(CoreNameDecoder::default()),
        );
    }

    #[test_log::test(tokio::test)]
    async fn vec_truncated() {
        let mut rx = FramedRead::new(
            b"\xff\xff\xff\xff\x0f\x03fo".as_slice(),
            CoreVecDecoder::<CoreNameDecoder>::default(),
        );
        rx.try_next().await.expect_err("truncated vec must fail");
    }

    #[test_log::test(tokio::test)]
    async fn bytes_truncated() {
        let mut rx = FramedRead::new(
            b"\xff\xff\xff\xff\x0ftest".as_slice(),
            CoreVecDecoderBytes::default(),
        );
        rx.try_next().await.expect_err("truncated bytes must fail");
    }
}
