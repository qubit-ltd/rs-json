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

use parse_display::{Display, FromStr as DeriveFromStr};

/// Represents the coarse category of a lenient JSON decoding failure.
///
/// This type is intended for callers that need stable, programmatic branching
/// without depending on full error messages produced by lower-level parsers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Display, DeriveFromStr)]
#[display(style = "snake_case")]
pub enum JsonDecodeErrorKind {
    /// Indicates that the raw input size exceeds the configured maximum.
    #[from_str(regex = "(?i)input_too_large")]
    InputTooLarge,
    /// Indicates that the input became empty after normalization.
    #[from_str(regex = "(?i)empty_input")]
    EmptyInput,
    /// Indicates that the normalized text is not valid JSON syntax.
    #[from_str(regex = "(?i)invalid_json")]
    InvalidJson,
    /// Indicates that the parsed top-level JSON kind is not the one required
    /// by the decoding method.
    #[from_str(regex = "(?i)unexpected_top_level")]
    UnexpectedTopLevel,
    /// Indicates that the JSON syntax is valid but the value cannot be
    /// deserialized into the requested Rust type.
    #[from_str(regex = "(?i)deserialize")]
    Deserialize,
}
