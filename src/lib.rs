//! Streaming WebAssembly codec based on Tokio
//!
//! Decoders are [`tokio_util::codec::Decoder`]s: `src` is the caller's buffer, which the caller
//! (e.g. [`tokio_util::codec::FramedRead`]) accumulates input in, and a decoder that needs more
//! bytes returns `Ok(None)`.
//!
//! A decoder error means the input is corrupt and decoding must stop: do not decode again with
//! the same decoder or buffer, just as [`tokio_util::codec::FramedRead`] ends the stream with
//! `None` after yielding the error.
//!
//! Length limits are opt-in: decoders constructed with `Default` or `new` accept any length the
//! encoding allows. Decoders constructed with `with_max_len` reject longer input with
//! [`std::io::ErrorKind::InvalidData`], and every nesting level needs its own limited decoder, e.g.
//! a vector of names needs both a limited [`CoreVecDecoder`] and a limited [`CoreNameDecoder`].

#![allow(clippy::module_name_repetitions)]

mod core;

/// [Component model](https://component-model.bytecodealliance.org/) codec
pub mod cm;

pub use core::*;
pub use leb128_tokio::*;
pub use utf8_tokio::*;

pub use tokio;
pub use tokio_util;
