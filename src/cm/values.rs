use ::core::future::Future;
use ::core::mem;

use leb128_tokio::{
    Leb128DecoderI16, Leb128DecoderI32, Leb128DecoderI64, Leb128DecoderU16, Leb128DecoderU32,
    Leb128DecoderU64, Leb128Encoder,
};
use tokio::io::{AsyncRead, AsyncReadExt as _, AsyncWrite, AsyncWriteExt as _};
use tokio_util::bytes::{Buf as _, BufMut as _, BytesMut};
use tokio_util::codec::{Decoder, Encoder};
use utf8_tokio::Utf8Codec;

use crate::CoreNameEncoder;

macro_rules! ensure_capacity {
    ($src:ident, $n:expr) => {
        if let Some(n) = $n.checked_sub($src.len()) {
            $src.reserve(n);
            return Ok(None);
        }
    };
}

macro_rules! impl_encode_copy_ref {
    ($enc:ident, $t:ty) => {
        impl Encoder<&$t> for $enc {
            type Error = std::io::Error;

            #[cfg_attr(
                       feature = "tracing",
                       tracing::instrument(level = "trace", ret, fields(ty = stringify!($t)))
            )]
            fn encode(&mut self, item: &$t, dst: &mut BytesMut) -> Result<(), Self::Error> {
                self.encode(*item, dst)
            }
        }
    };
}

macro_rules! impl_encode_str {
    ($enc:ident, $t:ty) => {
        impl Encoder<$t> for $enc {
            type Error = std::io::Error;

            #[cfg_attr(
                feature = "tracing",
                tracing::instrument(level = "trace", ret, fields(ty = "string"))
            )]
            fn encode(&mut self, item: $t, dst: &mut BytesMut) -> Result<(), Self::Error> {
                CoreNameEncoder.encode(item, dst)
            }
        }

        impl Encoder<&$t> for $enc {
            type Error = std::io::Error;

            #[cfg_attr(
                feature = "tracing",
                tracing::instrument(level = "trace", ret, fields(ty = "string"))
            )]
            fn encode(&mut self, item: &$t, dst: &mut BytesMut) -> Result<(), Self::Error> {
                CoreNameEncoder.encode(item, dst)
            }
        }
    };
}

pub trait AsyncReadValue: AsyncRead {
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(level = "trace", ret, skip_all, fields(ty = "bool"))
    )]
    fn read_bool(&mut self) -> impl Future<Output = std::io::Result<bool>>
    where
        Self: Unpin,
    {
        async move {
            match self.read_u8().await? {
                0 => Ok(false),
                1 => Ok(true),
                n => Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("invalid bool value byte `{n}`"),
                )),
            }
        }
    }

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(level = "trace", ret, skip_all, fields(ty = "option"))
    )]
    fn read_option_status(&mut self) -> impl Future<Output = std::io::Result<bool>>
    where
        Self: Unpin,
    {
        async move {
            match self.read_u8().await? {
                0 => Ok(false),
                1 => Ok(true),
                n => Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("invalid option status byte value `{n}`"),
                )),
            }
        }
    }

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(level = "trace", ret, skip_all, fields(ty = "result"))
    )]
    fn read_result_status(&mut self) -> impl Future<Output = std::io::Result<bool>>
    where
        Self: Unpin,
    {
        async move {
            match self.read_u8().await? {
                0 => Ok(true),
                1 => Ok(false),
                n => Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("invalid result status byte value `{n}`"),
                )),
            }
        }
    }
}

impl<T: AsyncRead> AsyncReadValue for T {}

pub trait AsyncWriteValue: AsyncWrite {
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(level = "trace", ret, skip_all, fields(ty = "bool"))
    )]
    fn write_bool(&mut self, v: bool) -> impl Future<Output = std::io::Result<()>>
    where
        Self: Unpin,
    {
        async move { self.write_u8(v.into()).await }
    }

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(level = "trace", ret, skip_all, fields(ty = "option"))
    )]
    fn write_option_status<T>(&mut self, v: Option<T>) -> impl Future<Output = std::io::Result<()>>
    where
        Self: Unpin,
    {
        async move { self.write_u8(v.is_some().into()).await }
    }

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(level = "trace", ret, skip_all, fields(ty = "result"))
    )]
    fn write_result_status<T, E>(
        &mut self,
        v: Result<T, E>,
    ) -> impl Future<Output = std::io::Result<()>>
    where
        Self: Unpin,
    {
        async move { self.write_u8(v.is_err().into()).await }
    }
}

impl<T: AsyncWrite> AsyncWriteValue for T {}

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct BoolCodec;

impl Encoder<bool> for BoolCodec {
    type Error = std::io::Error;

    #[cfg_attr(feature = "tracing", tracing::instrument(level = "trace", ret))]
    fn encode(&mut self, item: bool, dst: &mut BytesMut) -> Result<(), Self::Error> {
        dst.reserve(1);
        dst.put_u8(item.into());
        Ok(())
    }
}

impl Decoder for BoolCodec {
    type Item = bool;
    type Error = std::io::Error;

    #[cfg_attr(feature = "tracing", tracing::instrument(level = "trace", ret))]
    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        ensure_capacity!(src, 1_usize);
        match src.get_u8() {
            0 => Ok(Some(false)),
            1 => Ok(Some(true)),
            n => Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("invalid bool value byte `{n}`"),
            )),
        }
    }
}

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct S8Codec;

impl Encoder<i8> for S8Codec {
    type Error = std::io::Error;

    #[cfg_attr(feature = "tracing", tracing::instrument(level = "trace", ret))]
    fn encode(&mut self, item: i8, dst: &mut BytesMut) -> Result<(), Self::Error> {
        dst.reserve(1);
        dst.put_i8(item);
        Ok(())
    }
}

impl Decoder for S8Codec {
    type Item = i8;
    type Error = std::io::Error;

    #[cfg_attr(feature = "tracing", tracing::instrument(level = "trace", ret))]
    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        ensure_capacity!(src, 1_usize);
        Ok(Some(src.get_i8()))
    }
}

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct U8Codec;

impl Encoder<u8> for U8Codec {
    type Error = std::io::Error;

    #[cfg_attr(feature = "tracing", tracing::instrument(level = "trace", ret))]
    fn encode(&mut self, item: u8, dst: &mut BytesMut) -> Result<(), Self::Error> {
        dst.reserve(1);
        dst.put_u8(item);
        Ok(())
    }
}

impl_encode_copy_ref!(U8Codec, u8);

impl Decoder for U8Codec {
    type Item = u8;
    type Error = std::io::Error;

    #[cfg_attr(feature = "tracing", tracing::instrument(level = "trace", ret))]
    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        let Some(v) = src.first().copied() else {
            return Ok(None);
        };
        src.advance(1);
        Ok(Some(v))
    }
}

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct S16Codec;

impl Encoder<i16> for S16Codec {
    type Error = std::io::Error;

    #[cfg_attr(feature = "tracing", tracing::instrument(level = "trace", ret))]
    fn encode(&mut self, item: i16, dst: &mut BytesMut) -> Result<(), Self::Error> {
        Leb128Encoder.encode(item, dst)
    }
}

impl_encode_copy_ref!(S16Codec, i16);

impl Decoder for S16Codec {
    type Item = i16;
    type Error = std::io::Error;

    #[cfg_attr(feature = "tracing", tracing::instrument(level = "trace", ret))]
    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        Leb128DecoderI16.decode(src)
    }
}

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct U16Codec;

impl Encoder<u16> for U16Codec {
    type Error = std::io::Error;

    #[cfg_attr(feature = "tracing", tracing::instrument(level = "trace", ret))]
    fn encode(&mut self, item: u16, dst: &mut BytesMut) -> Result<(), Self::Error> {
        Leb128Encoder.encode(item, dst)
    }
}

impl_encode_copy_ref!(U16Codec, u16);

impl Decoder for U16Codec {
    type Item = u16;
    type Error = std::io::Error;

    #[cfg_attr(feature = "tracing", tracing::instrument(level = "trace", ret))]
    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        Leb128DecoderU16.decode(src)
    }
}

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct S32Codec;

impl Encoder<i32> for S32Codec {
    type Error = std::io::Error;

    #[cfg_attr(feature = "tracing", tracing::instrument(level = "trace", ret))]
    fn encode(&mut self, item: i32, dst: &mut BytesMut) -> Result<(), Self::Error> {
        Leb128Encoder.encode(item, dst)
    }
}

impl_encode_copy_ref!(S32Codec, i32);

impl Decoder for S32Codec {
    type Item = i32;
    type Error = std::io::Error;

    #[cfg_attr(feature = "tracing", tracing::instrument(level = "trace", ret))]
    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        Leb128DecoderI32.decode(src)
    }
}

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct U32Codec;

impl Encoder<u32> for U32Codec {
    type Error = std::io::Error;

    #[cfg_attr(feature = "tracing", tracing::instrument(level = "trace", ret))]
    fn encode(&mut self, item: u32, dst: &mut BytesMut) -> Result<(), Self::Error> {
        Leb128Encoder.encode(item, dst)
    }
}

impl_encode_copy_ref!(U32Codec, u32);

impl Decoder for U32Codec {
    type Item = u32;
    type Error = std::io::Error;

    #[cfg_attr(feature = "tracing", tracing::instrument(level = "trace", ret))]
    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        Leb128DecoderU32.decode(src)
    }
}

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct S64Codec;

impl Encoder<i64> for S64Codec {
    type Error = std::io::Error;

    #[cfg_attr(feature = "tracing", tracing::instrument(level = "trace", ret))]
    fn encode(&mut self, item: i64, dst: &mut BytesMut) -> Result<(), Self::Error> {
        Leb128Encoder.encode(item, dst)
    }
}

impl_encode_copy_ref!(S64Codec, i64);

impl Decoder for S64Codec {
    type Item = i64;
    type Error = std::io::Error;

    #[cfg_attr(feature = "tracing", tracing::instrument(level = "trace", ret))]
    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        Leb128DecoderI64.decode(src)
    }
}

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct U64Codec;

impl Encoder<u64> for U64Codec {
    type Error = std::io::Error;

    #[cfg_attr(feature = "tracing", tracing::instrument(level = "trace", ret))]
    fn encode(&mut self, item: u64, dst: &mut BytesMut) -> Result<(), Self::Error> {
        Leb128Encoder.encode(item, dst)
    }
}

impl_encode_copy_ref!(U64Codec, u64);

impl Decoder for U64Codec {
    type Item = u64;
    type Error = std::io::Error;

    #[cfg_attr(feature = "tracing", tracing::instrument(level = "trace", ret))]
    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        Leb128DecoderU64.decode(src)
    }
}

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct F32Codec;

impl Encoder<f32> for F32Codec {
    type Error = std::io::Error;

    #[cfg_attr(feature = "tracing", tracing::instrument(level = "trace", ret))]
    fn encode(&mut self, item: f32, dst: &mut BytesMut) -> Result<(), Self::Error> {
        dst.reserve(4);
        dst.put_f32_le(item);
        Ok(())
    }
}

impl_encode_copy_ref!(F32Codec, f32);

impl Decoder for F32Codec {
    type Item = f32;
    type Error = std::io::Error;

    #[cfg_attr(feature = "tracing", tracing::instrument(level = "trace", ret))]
    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        ensure_capacity!(src, 4_usize);
        Ok(Some(src.get_f32_le()))
    }
}

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct F64Codec;

impl Encoder<f64> for F64Codec {
    type Error = std::io::Error;

    #[cfg_attr(feature = "tracing", tracing::instrument(level = "trace", ret))]
    fn encode(&mut self, item: f64, dst: &mut BytesMut) -> Result<(), Self::Error> {
        dst.reserve(8);
        dst.put_f64_le(item);
        Ok(())
    }
}

impl_encode_copy_ref!(F64Codec, f64);

impl Decoder for F64Codec {
    type Item = f64;
    type Error = std::io::Error;

    #[cfg_attr(feature = "tracing", tracing::instrument(level = "trace", ret))]
    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        ensure_capacity!(src, 8_usize);
        Ok(Some(src.get_f64_le()))
    }
}

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct PrimValEncoder;

impl Encoder<bool> for PrimValEncoder {
    type Error = std::io::Error;

    fn encode(&mut self, item: bool, dst: &mut BytesMut) -> Result<(), Self::Error> {
        BoolCodec.encode(item, dst)
    }
}

impl Encoder<i8> for PrimValEncoder {
    type Error = std::io::Error;

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(level = "trace", ret, fields(ty = "s8"))
    )]
    fn encode(&mut self, item: i8, dst: &mut BytesMut) -> Result<(), Self::Error> {
        S8Codec.encode(item, dst)
    }
}

impl Encoder<u8> for PrimValEncoder {
    type Error = std::io::Error;

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(level = "trace", ret, fields(ty = "u8"))
    )]
    fn encode(&mut self, item: u8, dst: &mut BytesMut) -> Result<(), Self::Error> {
        U8Codec.encode(item, dst)
    }
}

impl Encoder<i16> for PrimValEncoder {
    type Error = std::io::Error;

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(level = "trace", ret, fields(ty = "s16"))
    )]
    fn encode(&mut self, item: i16, dst: &mut BytesMut) -> Result<(), Self::Error> {
        S16Codec.encode(item, dst)
    }
}

impl Encoder<u16> for PrimValEncoder {
    type Error = std::io::Error;

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(level = "trace", ret, fields(ty = "u16"))
    )]
    fn encode(&mut self, item: u16, dst: &mut BytesMut) -> Result<(), Self::Error> {
        U16Codec.encode(item, dst)
    }
}

impl Encoder<i32> for PrimValEncoder {
    type Error = std::io::Error;

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(level = "trace", ret, fields(ty = "s32"))
    )]
    fn encode(&mut self, item: i32, dst: &mut BytesMut) -> Result<(), Self::Error> {
        S32Codec.encode(item, dst)
    }
}

impl Encoder<u32> for PrimValEncoder {
    type Error = std::io::Error;

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(level = "trace", ret, fields(ty = "u32"))
    )]
    fn encode(&mut self, item: u32, dst: &mut BytesMut) -> Result<(), Self::Error> {
        U32Codec.encode(item, dst)
    }
}

impl Encoder<i64> for PrimValEncoder {
    type Error = std::io::Error;

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(level = "trace", ret, fields(ty = "s64"))
    )]
    fn encode(&mut self, item: i64, dst: &mut BytesMut) -> Result<(), Self::Error> {
        S64Codec.encode(item, dst)
    }
}

impl Encoder<u64> for PrimValEncoder {
    type Error = std::io::Error;

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(level = "trace", ret, fields(ty = "u64"))
    )]
    fn encode(&mut self, item: u64, dst: &mut BytesMut) -> Result<(), Self::Error> {
        U64Codec.encode(item, dst)
    }
}

impl Encoder<f32> for PrimValEncoder {
    type Error = std::io::Error;

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(level = "trace", ret, fields(ty = "f32"))
    )]
    fn encode(&mut self, item: f32, dst: &mut BytesMut) -> Result<(), Self::Error> {
        F32Codec.encode(item, dst)
    }
}

impl Encoder<f64> for PrimValEncoder {
    type Error = std::io::Error;

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(level = "trace", ret, fields(ty = "f64"))
    )]
    fn encode(&mut self, item: f64, dst: &mut BytesMut) -> Result<(), Self::Error> {
        F64Codec.encode(item, dst)
    }
}

impl Encoder<char> for PrimValEncoder {
    type Error = std::io::Error;

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(level = "trace", ret, fields(ty = "char"))
    )]
    fn encode(&mut self, item: char, dst: &mut BytesMut) -> Result<(), Self::Error> {
        Utf8Codec.encode(item, dst)
    }
}

impl_encode_copy_ref!(PrimValEncoder, bool);
impl_encode_copy_ref!(PrimValEncoder, i8);
impl_encode_copy_ref!(PrimValEncoder, u8);
impl_encode_copy_ref!(PrimValEncoder, i16);
impl_encode_copy_ref!(PrimValEncoder, u16);
impl_encode_copy_ref!(PrimValEncoder, i32);
impl_encode_copy_ref!(PrimValEncoder, u32);
impl_encode_copy_ref!(PrimValEncoder, i64);
impl_encode_copy_ref!(PrimValEncoder, u64);
impl_encode_copy_ref!(PrimValEncoder, f32);
impl_encode_copy_ref!(PrimValEncoder, f64);
impl_encode_copy_ref!(PrimValEncoder, char);

impl_encode_str!(PrimValEncoder, &str);
impl_encode_str!(PrimValEncoder, String);

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct TupleEncoder<T>(pub T);

impl<T> From<(T,)> for TupleEncoder<(T,)> {
    fn from((e,): (T,)) -> Self {
        Self((e,))
    }
}

impl<C, V> Encoder<(V,)> for TupleEncoder<(C,)>
where
    C: Encoder<V>,
{
    type Error = C::Error;

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(level = "trace", skip_all, fields(dst, ty = "tuple"))
    )]
    fn encode(&mut self, (v,): (V,), dst: &mut BytesMut) -> Result<(), Self::Error> {
        let (ref mut e,) = self.0;
        e.encode(v, dst)?;
        Ok(())
    }
}

#[derive(Debug)]
pub struct TupleDecoder<C, V> {
    dec: C,
    v: V,
}

impl<C, V> TupleDecoder<C, V> {
    pub fn into_inner(TupleDecoder { dec, .. }: Self) -> C {
        dec
    }
}

impl<C, V> TupleDecoder<C, V>
where
    V: Default,
{
    pub fn new(decoder: C) -> Self {
        Self {
            dec: decoder,
            v: V::default(),
        }
    }
}

impl<C, V> Default for TupleDecoder<C, V>
where
    C: Default,
    V: Default,
{
    fn default() -> Self {
        Self::new(C::default())
    }
}

impl<C> Decoder for TupleDecoder<(C,), (Option<C::Item>,)>
where
    C: Decoder,
{
    type Item = (C::Item,);
    type Error = C::Error;

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(level = "trace", skip(self), fields(ty = "tuple"))
    )]
    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        let (ref mut v,) = self.v;
        let (ref mut d,) = self.dec;
        if v.is_none() {
            let Some(dv) = d.decode(src)? else {
                return Ok(None);
            };
            *v = Some(dv);
        }
        Ok(Some((mem::take(v).unwrap(),)))
    }
}

macro_rules! impl_tuple_codec {
    ($($vn:ident),+; $($vt:ident),+; $($cn:ident),+; $($ct:ident),+) => {
        impl<$($ct),+> From<($($ct),+)> for TupleEncoder<($($ct),+)> {
            fn from(e: ($($ct),+)) -> Self {
               Self(e)
            }
        }

        impl<E, $($vt, $ct),+> Encoder<($($vt),+)> for TupleEncoder<($($ct),+)>
        where
            E: From<std::io::Error>,
            $($ct: Encoder<$vt, Error = E>),+
        {
            type Error = E;

            #[cfg_attr(
                feature = "tracing",
                tracing::instrument(level = "trace", skip_all, fields(dst, ty = "tuple"))
            )]
            fn encode(
                &mut self,
                ($($vn),+): ($($vt),+),
                dst: &mut BytesMut,
            ) -> Result<(), Self::Error> {
                    let ($(ref mut $cn),+) = self.0;
                    $($cn.encode($vn, dst)?;)+
                    Ok(())
            }
        }

        impl<'a, E, $($vt, $ct),+> Encoder<&'a ($($vt),+)> for TupleEncoder<($($ct),+)>
        where
            E: From<std::io::Error>,
            $($ct: Encoder<&'a $vt, Error = E>),+
        {
            type Error = E;

            #[cfg_attr(
                feature = "tracing",
                tracing::instrument(level = "trace", skip_all, fields(dst, ty = "tuple"))
            )]
            fn encode(
                &mut self,
                ($($vn),+): &'a ($($vt),+),
                dst: &mut BytesMut,
            ) -> Result<(), Self::Error> {
                    let ($(ref mut $cn),+) = self.0;
                    $($cn.encode($vn, dst)?;)+
                    Ok(())
            }
        }

        impl<E, $($ct),+> Decoder for TupleDecoder<($($ct),+), ($(Option<$ct::Item>),+)>
        where
            E: From<std::io::Error>,
            $($ct: Decoder<Error = E>),+
        {
            type Error = E;
            type Item = ($($ct::Item),+);

            #[cfg_attr(
                feature = "tracing",
                tracing::instrument(level = "trace", skip(self), fields(ty = "tuple"))
            )]
            fn decode(
                &mut self,
                src: &mut BytesMut,
            ) -> Result<Option<Self::Item>, Self::Error> {
                    let ($(ref mut $vn),+) = self.v;
                    let ($(ref mut $cn),+) = self.dec;
                    $(
                        if $vn.is_none() {
                            let Some(v) = $cn.decode(src)? else  {
                                return Ok(None)
                            };
                            *$vn = Some(v);
                        }
                    )+
                    Ok(Some(($(core::mem::take($vn).unwrap()),+)))
            }
        }
    };
}

impl_tuple_codec!(
    v0, v1;
    V0, V1;
    c0, c1;
    C0, C1
);

impl_tuple_codec!(
    v0, v1, v2;
    V0, V1, V2;
    c0, c1, c2;
    C0, C1, C2
);

impl_tuple_codec!(
    v0, v1, v2, v3;
    V0, V1, V2, V3;
    c0, c1, c2, c3;
    C0, C1, C2, C3
);

impl_tuple_codec!(
    v0, v1, v2, v3, v4;
    V0, V1, V2, V3, V4;
    c0, c1, c2, c3, c4;
    C0, C1, C2, C3, C4
);

impl_tuple_codec!(
    v0, v1, v2, v3, v4, v5;
    V0, V1, V2, V3, V4, V5;
    c0, c1, c2, c3, c4, c5;
    C0, C1, C2, C3, C4, C5
);

impl_tuple_codec!(
    v0, v1, v2, v3, v4, v5, v6;
    V0, V1, V2, V3, V4, V5, V6;
    c0, c1, c2, c3, c4, c5, c6;
    C0, C1, C2, C3, C4, C5, C6
);

impl_tuple_codec!(
    v0, v1, v2, v3, v4, v5, v6, v7;
    V0, V1, V2, V3, V4, V5, V6, V7;
    c0, c1, c2, c3, c4, c5, c6, c7;
    C0, C1, C2, C3, C4, C5, C6, C7
);

impl_tuple_codec!(
    v0, v1, v2, v3, v4, v5, v6, v7, v8;
    V0, V1, V2, V3, V4, V5, V6, V7, V8;
    c0, c1, c2, c3, c4, c5, c6, c7, c8;
    C0, C1, C2, C3, C4, C5, C6, C7, C8
);

impl_tuple_codec!(
    v0, v1, v2, v3, v4, v5, v6, v7, v8, v9;
    V0, V1, V2, V3, V4, V5, V6, V7, V8, V9;
    c0, c1, c2, c3, c4, c5, c6, c7, c8, c9;
    C0, C1, C2, C3, C4, C5, C6, C7, C8, C9
);

impl_tuple_codec!(
    v0, v1, v2, v3, v4, v5, v6, v7, v8, v9, v10;
    V0, V1, V2, V3, V4, V5, V6, V7, V8, V9, V10;
    c0, c1, c2, c3, c4, c5, c6, c7, c8, c9, c10;
    C0, C1, C2, C3, C4, C5, C6, C7, C8, C9, C10
);

impl_tuple_codec!(
    v0, v1, v2, v3, v4, v5, v6, v7, v8, v9, v10, v11;
    V0, V1, V2, V3, V4, V5, V6, V7, V8, V9, V10, V11;
    c0, c1, c2, c3, c4, c5, c6, c7, c8, c9, c10, c11;
    C0, C1, C2, C3, C4, C5, C6, C7, C8, C9, C10, C11
);

impl_tuple_codec!(
    v0, v1, v2, v3, v4, v5, v6, v7, v8, v9, v10, v11, v12;
    V0, V1, V2, V3, V4, V5, V6, V7, V8, V9, V10, V11, V12;
    c0, c1, c2, c3, c4, c5, c6, c7, c8, c9, c10, c11, c12;
    C0, C1, C2, C3, C4, C5, C6, C7, C8, C9, C10, C11, C12
);

impl_tuple_codec!(
    v0, v1, v2, v3, v4, v5, v6, v7, v8, v9, v10, v11, v12, v13;
    V0, V1, V2, V3, V4, V5, V6, V7, V8, V9, V10, V11, V12, V13;
    c0, c1, c2, c3, c4, c5, c6, c7, c8, c9, c10, c11, c12, c13;
    C0, C1, C2, C3, C4, C5, C6, C7, C8, C9, C10, C11, C12, C13
);

impl_tuple_codec!(
    v0, v1, v2, v3, v4, v5, v6, v7, v8, v9, v10, v11, v12, v13, v14;
    V0, V1, V2, V3, V4, V5, V6, V7, V8, V9, V10, V11, V12, V13, V14;
    c0, c1, c2, c3, c4, c5, c6, c7, c8, c9, c10, c11, c12, c13, c14;
    C0, C1, C2, C3, C4, C5, C6, C7, C8, C9, C10, C11, C12, C13, C14
);

impl_tuple_codec!(
    v0, v1, v2, v3, v4, v5, v6, v7, v8, v9, v10, v11, v12, v13, v14, v15;
    V0, V1, V2, V3, V4, V5, V6, V7, V8, V9, V10, V11, V12, V13, V14, V15;
    c0, c1, c2, c3, c4, c5, c6, c7, c8, c9, c10, c11, c12, c13, c14, c15;
    C0, C1, C2, C3, C4, C5, C6, C7, C8, C9, C10, C11, C12, C13, C14, C15
);

#[cfg(test)]
mod tests {
    use crate::CoreNameDecoder;

    use super::*;

    #[test_log::test]
    fn tuple() {
        let mut buf = BytesMut::default();
        TupleEncoder((BoolCodec, PrimValEncoder, CoreNameEncoder, Leb128Encoder))
            .encode((true, 0xfeu8, "test", 0x42u32), &mut buf)
            .expect("failed to encode tuple");
        assert_eq!(buf.as_ref(), b"\x01\xfe\x04test\x42");
        let (a, b, c, d) = TupleDecoder::new((
            BoolCodec,
            U8Codec,
            CoreNameDecoder::default(),
            Leb128DecoderU32,
        ))
        .decode(&mut buf)
        .expect("failed to decode tuple")
        .expect("short tuple read");
        assert!(a);
        assert_eq!(b, 0xfe);
        assert_eq!(c, "test");
        assert_eq!(d, 0x42);
    }
}
