/*******************************************************************************
 *
 *    Copyright (c) 2026.
 *    Haixing Hu, Qubit Co. Ltd.
 *
 *    All rights reserved.
 *
 ******************************************************************************/
//! Provides the public API for the `qubit-json` crate.
//!
//! The crate exposes a lenient JSON decoder and the related option and error
//! types needed to normalize and deserialize JSON text from
//! non-fully-trusted sources.
//!
//! Author: Haixing Hu

#![deny(missing_docs)]

mod decode_error_kind;
mod decoder;
mod error;
mod normalize;
mod options;
mod top_level_kind;

pub use decode_error_kind::JsonDecodeErrorKind;
pub use decoder::LenientJsonDecoder;
pub use error::JsonDecodeError;
pub use options::LenientJsonDecoderOptions;
pub use top_level_kind::JsonTopLevelKind;
