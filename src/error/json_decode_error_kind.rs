// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines the stable error categories returned by the decoder.

use std::fmt;
use std::str::FromStr;

/// Represents the coarse category of a lenient JSON decoding failure.
///
/// This type is intended for callers that need stable, programmatic branching
/// without depending on full error messages produced by lower-level parsers.
///
/// # Examples
///
/// ```compile_fail
/// #![deny(unused_must_use)]
/// use qubit_json::JsonDecodeErrorKind;
///
/// fn error_kind() -> JsonDecodeErrorKind {
///     JsonDecodeErrorKind::InvalidJson
/// }
///
/// error_kind();
/// ```
#[must_use]
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JsonDecodeErrorKind {
    /// Indicates that raw or normalized input size exceeds a configured
    /// maximum.
    InputTooLarge,
    /// Indicates that the input became empty after normalization.
    EmptyInput,
    /// Indicates that the raw byte input is not valid UTF-8 text.
    InvalidUtf8,
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
    /// Writes the stable snake-case name of this error category.
    ///
    /// # Parameters
    ///
    /// * `f` - Destination formatter.
    ///
    /// # Returns
    ///
    /// `Ok(())` when the category name is written successfully.
    ///
    /// # Errors
    ///
    /// Returns a formatting error when the destination rejects the write.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::InputTooLarge => "input_too_large",
            Self::EmptyInput => "empty_input",
            Self::InvalidUtf8 => "invalid_utf8",
            Self::InvalidJson => "invalid_json",
            Self::UnexpectedTopLevel => "unexpected_top_level",
            Self::Deserialize => "deserialize",
        };
        f.write_str(name)
    }
}

impl FromStr for JsonDecodeErrorKind {
    type Err = &'static str;

    /// Parses a stable snake-case error category without ASCII case
    /// sensitivity.
    ///
    /// # Parameters
    ///
    /// * `value` - Category name to parse.
    ///
    /// # Returns
    ///
    /// The matching error category.
    ///
    /// # Errors
    ///
    /// Returns a static diagnostic when `value` is not a known category name.
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.eq_ignore_ascii_case("input_too_large") {
            Ok(Self::InputTooLarge)
        } else if value.eq_ignore_ascii_case("empty_input") {
            Ok(Self::EmptyInput)
        } else if value.eq_ignore_ascii_case("invalid_utf8") {
            Ok(Self::InvalidUtf8)
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
