use ::core::future::Future;
use ::core::mem;
use ::core::str;

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
            s.reserve(n.try_into().unwrap_or(usize::MAX));
            self.take(n.into()).read_to_string(s).await?;
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

impl Encoder<&str> for CoreNameEncoder {
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

impl Encoder<&&str> for CoreNameEncoder {
    type Error = std::io::Error;

    fn encode(&mut self, item: &&str, dst: &mut BytesMut) -> Result<(), Self::Error> {
        self.encode(*item, dst)
    }
}

impl Encoder<String> for CoreNameEncoder {
    type Error = std::io::Error;

    fn encode(&mut self, item: String, dst: &mut BytesMut) -> Result<(), Self::Error> {
        self.encode(item.as_str(), dst)
    }
}

impl Encoder<&String> for CoreNameEncoder {
    type Error = std::io::Error;

    fn encode(&mut self, item: &String, dst: &mut BytesMut) -> Result<(), Self::Error> {
        self.encode(item.as_str(), dst)
    }
}

/// [`core:name`](https://webassembly.github.io/spec/core/binary/values.html#names) decoder
#[derive(Debug, Default)]
pub struct CoreNameDecoder(CoreVecDecoderBytes);

impl Decoder for CoreNameDecoder {
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

impl<'a, E, T> Encoder<&'a [T]> for CoreVecEncoder<E>
where
    E: Encoder<&'a T>,
    E::Error: Into<std::io::Error>,
{
    type Error = std::io::Error;

    fn encode(&mut self, item: &'a [T], dst: &mut BytesMut) -> Result<(), Self::Error> {
        let len = item.len();
        dst.reserve(5 + len);
        let len = u32::try_from(len)
            .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidInput, err))?;
        Leb128Encoder.encode(len, dst)?;
        for item in item {
            self.0.encode(item, dst).map_err(Into::into)?;
        }
        Ok(())
    }
}

/// [`core:vec`](https://webassembly.github.io/spec/core/binary/conventions.html#binary-vec) decoder
#[derive(Debug)]
pub struct CoreVecDecoder<T: Decoder> {
    dec: T,
    ret: Vec<T::Item>,
    cap: usize,
}

impl<T> CoreVecDecoder<T>
where
    T: Decoder,
{
    pub fn new(decoder: T) -> Self {
        Self {
            dec: decoder,
            ret: Vec::default(),
            cap: 0,
        }
    }

    pub fn into_inner(CoreVecDecoder { dec, .. }: Self) -> T {
        dec
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

impl<T> Decoder for CoreVecDecoder<T>
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
            self.ret = Vec::with_capacity(len);
            self.cap = len;
        }
        while self.cap > 0 {
            let Some(v) = self.dec.decode(src)? else {
                return Ok(None);
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

impl Encoder<&[u8]> for CoreVecEncoderBytes {
    type Error = std::io::Error;

    fn encode(&mut self, item: &[u8], dst: &mut BytesMut) -> Result<(), Self::Error> {
        let n = item.len();
        let n = u32::try_from(n)
            .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidInput, err))?;
        dst.reserve(item.len().saturating_add(5));
        Leb128Encoder.encode(n, dst)?;
        dst.extend_from_slice(item);
        Ok(())
    }
}

impl Encoder<Vec<u8>> for CoreVecEncoderBytes {
    type Error = std::io::Error;

    fn encode(&mut self, item: Vec<u8>, dst: &mut BytesMut) -> Result<(), Self::Error> {
        let item: &[u8] = item.as_ref();
        self.encode(item, dst)
    }
}

impl Encoder<Bytes> for CoreVecEncoderBytes {
    type Error = std::io::Error;

    fn encode(&mut self, item: Bytes, dst: &mut BytesMut) -> Result<(), Self::Error> {
        let item: &[u8] = item.as_ref();
        self.encode(item, dst)
    }
}

/// [`core:vec`](https://webassembly.github.io/spec/core/binary/conventions.html#binary-vec)
/// decoder optimized for vectors of byte-sized values
#[derive(Debug, Default)]
pub struct CoreVecDecoderBytes(usize);

impl Decoder for CoreVecDecoderBytes {
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
        let n = self.0.saturating_sub(src.len());
        if n > 0 {
            src.reserve(n);
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
        tx.send("").await.expect("failed to send ``");

        trace!("sending `test`");
        tx.send("test").await.expect("failed to send `test`");

        trace!("sending `bar`");
        tx.send("bar").await.expect("failed to send `bar`");

        trace!("sending `ƒ𐍈Ő`");
        tx.send("ƒ𐍈Ő").await.expect("failed to send `ƒ𐍈Ő`");

        trace!("sending `baz`");
        tx.send("baz").await.expect("failed to send `baz`");

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
}
