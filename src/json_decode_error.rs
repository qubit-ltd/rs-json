/*******************************************************************************
 *
 *    Copyright (c) 2026.
 *    Haixing Hu, Qubit Co. Ltd.
 *
 *    All rights reserved.
 *
 ******************************************************************************/
//! Defines the [`JsonDecodeError`] type used by the public decoder API.
//!
//! Author: Haixing Hu

use std::fmt;

use crate::{JsonDecodeErrorKind, JsonTopLevelKind};

/// Identifies the decoding stage where an error was produced.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JsonDecodeStage {
    /// The error happened while normalizing raw input text.
    Normalize,
    /// The error happened while parsing normalized text as JSON syntax.
    Parse,
    /// The error happened while enforcing a top-level kind contract.
    TopLevelCheck,
    /// The error happened while deserializing a parsed JSON value.
    Deserialize,
}

impl fmt::Display for JsonDecodeStage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Normalize => f.write_str("normalize"),
            Self::Parse => f.write_str("parse"),
            Self::TopLevelCheck => f.write_str("top_level_check"),
            Self::Deserialize => f.write_str("deserialize"),
        }
    }
}

/// Error returned when lenient JSON decoding fails.
///
/// This value captures both a stable category in [`JsonDecodeErrorKind`] and
/// human-readable context that can be logged or surfaced to the caller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonDecodeError {
    /// Identifies the stable category of this decoding failure.
    ///
    /// Callers should match on this field when they need programmatic handling
    /// that is independent from localized or parser-specific text.
    pub kind: JsonDecodeErrorKind,
    /// Identifies which decode stage produced this error.
    pub stage: JsonDecodeStage,
    /// Stores a human-readable summary of the decoding failure.
    ///
    /// The message is intended for diagnostics and normally includes the
    /// relevant parsing or deserialization context.
    pub message: String,
    /// Stores the top-level JSON kind required by the caller, when applicable.
    ///
    /// This field is only populated for errors raised by constrained decoding
    /// methods such as `decode_object()` and `decode_array()`.
    pub expected_top_level: Option<JsonTopLevelKind>,
    /// Stores the top-level JSON kind that was actually parsed, when known.
    ///
    /// This field is only populated together with `expected_top_level` for
    /// top-level contract mismatches.
    pub actual_top_level: Option<JsonTopLevelKind>,
    /// Stores the one-based line reported by `serde_json`, when available.
    ///
    /// This field is primarily useful for invalid JSON syntax and
    /// deserialization failures that can be mapped back to a parser location.
    pub line: Option<usize>,
    /// Stores the one-based column reported by `serde_json`, when available.
    ///
    /// Like `line`, this field is only populated when the lower-level parser
    /// or deserializer reports a concrete source position.
    pub column: Option<usize>,
    /// Stores the input byte length associated with the failure, when known.
    pub input_bytes: Option<usize>,
    /// Stores the configured maximum input byte length, when relevant.
    pub max_input_bytes: Option<usize>,
}

impl JsonDecodeError {
    /// Creates an error indicating that the raw input size exceeds a
    /// configured upper bound.
    #[inline]
    pub(crate) fn input_too_large(actual_bytes: usize, max_bytes: usize) -> Self {
        Self {
            kind: JsonDecodeErrorKind::InputTooLarge,
            stage: JsonDecodeStage::Normalize,
            message: format!(
                "JSON input is too large: {} bytes exceed configured limit {} bytes",
                actual_bytes, max_bytes
            ),
            expected_top_level: None,
            actual_top_level: None,
            line: None,
            column: None,
            input_bytes: Some(actual_bytes),
            max_input_bytes: Some(max_bytes),
        }
    }

    /// Creates an error indicating that the input became empty after
    /// normalization.
    #[inline]
    pub(crate) fn empty_input() -> Self {
        Self {
            kind: JsonDecodeErrorKind::EmptyInput,
            stage: JsonDecodeStage::Normalize,
            message: "JSON input is empty after normalization".to_string(),
            expected_top_level: None,
            actual_top_level: None,
            line: None,
            column: None,
            input_bytes: None,
            max_input_bytes: None,
        }
    }

    /// Creates an error describing invalid JSON syntax reported by
    /// `serde_json`.
    #[inline]
    pub(crate) fn invalid_json(error: serde_json::Error, input_bytes: Option<usize>) -> Self {
        let line = error.line();
        let column = error.column();
        Self {
            kind: JsonDecodeErrorKind::InvalidJson,
            stage: JsonDecodeStage::Parse,
            message: format!("Failed to parse JSON: {error}"),
            expected_top_level: None,
            actual_top_level: None,
            line: (line > 0).then_some(line),
            column: (column > 0).then_some(column),
            input_bytes,
            max_input_bytes: None,
        }
    }

    /// Creates an error describing a mismatch between expected and actual
    /// top-level JSON kinds.
    #[inline]
    pub(crate) fn unexpected_top_level(
        expected: JsonTopLevelKind,
        actual: JsonTopLevelKind,
    ) -> Self {
        Self {
            kind: JsonDecodeErrorKind::UnexpectedTopLevel,
            stage: JsonDecodeStage::TopLevelCheck,
            message: format!("Unexpected JSON top-level type: expected {expected}, got {actual}"),
            expected_top_level: Some(expected),
            actual_top_level: Some(actual),
            line: None,
            column: None,
            input_bytes: None,
            max_input_bytes: None,
        }
    }

    /// Creates an error describing a type deserialization failure reported by
    /// `serde_json`.
    #[inline]
    pub(crate) fn deserialize(error: serde_json::Error, input_bytes: Option<usize>) -> Self {
        let line = error.line();
        let column = error.column();
        Self {
            kind: JsonDecodeErrorKind::Deserialize,
            stage: JsonDecodeStage::Deserialize,
            message: format!("Failed to deserialize JSON value: {error}"),
            expected_top_level: None,
            actual_top_level: None,
            line: (line > 0).then_some(line),
            column: (column > 0).then_some(column),
            input_bytes,
            max_input_bytes: None,
        }
    }
}

impl fmt::Display for JsonDecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            JsonDecodeErrorKind::InputTooLarge => f.write_str(&self.message),
            JsonDecodeErrorKind::EmptyInput => f.write_str(&self.message),
            JsonDecodeErrorKind::UnexpectedTopLevel => f.write_str(&self.message),
            JsonDecodeErrorKind::InvalidJson | JsonDecodeErrorKind::Deserialize => {
                match (self.line, self.column) {
                    (Some(line), Some(column)) => {
                        write!(f, "{} (line {}, column {})", self.message, line, column)
                    }
                    _ => f.write_str(&self.message),
                }
            }
        }
    }
}

impl std::error::Error for JsonDecodeError {}
