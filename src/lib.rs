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

mod json_decode_error;
mod json_decode_error_kind;
mod json_decode_options;
mod json_top_level_kind;
mod lenient_json_decoder;
mod lenient_json_normalizer;

pub use json_decode_error::{JsonDecodeError, JsonDecodeStage};
pub use json_decode_error_kind::JsonDecodeErrorKind;
pub use json_decode_options::JsonDecodeOptions;
pub use json_top_level_kind::JsonTopLevelKind;
pub use lenient_json_decoder::LenientJsonDecoder;
pub use lenient_json_normalizer::LenientJsonNormalizer;
