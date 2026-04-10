/*******************************************************************************
 *
 *    Copyright (c) 2026.
 *    Haixing Hu, Qubit Co. Ltd.
 *
 *    All rights reserved.
 *
 ******************************************************************************/
//! Defines the stable error categories returned by the decoder.
//!
//! Author: Haixing Hu

use std::fmt;

/// Represents the coarse category of a lenient JSON decoding failure.
///
/// This type is intended for callers that need stable, programmatic branching
/// without depending on full error messages produced by lower-level parsers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JsonDecodeErrorKind {
    /// Indicates that the input became empty after normalization.
    EmptyInput,
    /// Indicates that the normalized text is not valid JSON syntax.
    InvalidJson,
    /// Indicates that the parsed top-level JSON kind is not the one required
    /// by the decoding method.
    UnexpectedTopLevel,
    /// Indicates that the JSON syntax is valid but the value cannot be
    /// deserialized into the requested Rust type.
    Deserialize,
}

impl fmt::Display for JsonDecodeErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            Self::EmptyInput => "empty_input",
            Self::InvalidJson => "invalid_json",
            Self::UnexpectedTopLevel => "unexpected_top_level",
            Self::Deserialize => "deserialize",
        };
        f.write_str(text)
    }
}
