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

use std::{fmt, str::FromStr};

/// Represents the coarse category of a lenient JSON decoding failure.
///
/// This type is intended for callers that need stable, programmatic branching
/// without depending on full error messages produced by lower-level parsers.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JsonDecodeErrorKind {
    /// Indicates that the raw input size exceeds the configured maximum.
    InputTooLarge,
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
        let name = match self {
            Self::InputTooLarge => "input_too_large",
            Self::EmptyInput => "empty_input",
            Self::InvalidJson => "invalid_json",
            Self::UnexpectedTopLevel => "unexpected_top_level",
            Self::Deserialize => "deserialize",
        };
        f.write_str(name)
    }
}

impl FromStr for JsonDecodeErrorKind {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.eq_ignore_ascii_case("input_too_large") {
            Ok(Self::InputTooLarge)
        } else if value.eq_ignore_ascii_case("empty_input") {
            Ok(Self::EmptyInput)
        } else if value.eq_ignore_ascii_case("invalid_json") {
            Ok(Self::InvalidJson)
        } else if value.eq_ignore_ascii_case("unexpected_top_level") {
            Ok(Self::UnexpectedTopLevel)
        } else if value.eq_ignore_ascii_case("deserialize") {
            Ok(Self::Deserialize)
        } else {
            Err("unknown JsonDecodeErrorKind")
        }
    }
}
