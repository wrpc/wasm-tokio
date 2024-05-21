use ::core::future::Future;

use tokio::io::{AsyncRead, AsyncReadExt as _, AsyncWrite, AsyncWriteExt as _};

use crate::core::{AsyncReadCore as _, AsyncWriteCore as _};

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

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(level = "trace", ret, skip_all, fields(ty = "char"))
    )]
    fn read_char(&mut self) -> impl Future<Output = std::io::Result<char>>
    where
        Self: Unpin + Sized,
    {
        async move {
            let n = self.read_u32_leb128().await?;
            char::from_u32(n).ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("invalid character value `{n}`"),
                )
            })
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

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(level = "trace", ret, skip_all, fields(ty = "char"))
    )]
    fn write_char<T, E>(&mut self, v: char) -> impl Future<Output = std::io::Result<usize>>
    where
        Self: Unpin + Sized,
    {
        async move { self.write_u32_leb128(v.into()).await }
    }
}

impl<T: AsyncWrite> AsyncWriteValue for T {}