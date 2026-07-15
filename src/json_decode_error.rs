// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines the [`JsonDecodeError`] type used by the public decoder API.

use std::{
    fmt,
    sync::Arc,
};

use crate::{
    JsonDecodeErrorKind,
    JsonDecodeStage,
    JsonTopLevelKind,
};

/// Error returned when lenient JSON decoding fails.
///
/// This type exposes immutable diagnostics so its error category, stage, and
/// associated input metadata always remain consistent with one another.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct JsonDecodeError {
    kind: JsonDecodeErrorKind,
    stage: JsonDecodeStage,
    message: String,
    expected_top_level: Option<JsonTopLevelKind>,
    actual_top_level: Option<JsonTopLevelKind>,
    raw_input_bytes: usize,
    normalized_input_bytes: Option<usize>,
    max_input_bytes: Option<usize>,
    normalized_line: Option<usize>,
    normalized_column: Option<usize>,
    source: Option<Arc<serde_json::Error>>,
}

impl JsonDecodeError {
    /// Returns the stable category of this decoding failure.
    #[must_use]
    pub const fn kind(&self) -> JsonDecodeErrorKind {
        self.kind
    }

    /// Returns the decoding stage that produced this error.
    #[must_use]
    pub const fn stage(&self) -> JsonDecodeStage {
        self.stage
    }

    /// Returns the human-readable diagnostic message.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Returns the required top-level JSON kind, when constrained decoding
    /// rejected a valid value.
    #[must_use]
    pub const fn expected_top_level(&self) -> Option<JsonTopLevelKind> {
        self.expected_top_level
    }

    /// Returns the parsed top-level JSON kind, when constrained decoding
    /// rejected a valid value.
    #[must_use]
    pub const fn actual_top_level(&self) -> Option<JsonTopLevelKind> {
        self.actual_top_level
    }

    /// Returns the byte length of the input before normalization.
    #[must_use]
    pub const fn raw_input_bytes(&self) -> usize {
        self.raw_input_bytes
    }

    /// Returns the byte length of normalized JSON text, when normalization
    /// completed before the failure.
    #[must_use]
    pub const fn normalized_input_bytes(&self) -> Option<usize> {
        self.normalized_input_bytes
    }

    /// Returns the configured raw-input limit for size failures.
    #[must_use]
    pub const fn max_input_bytes(&self) -> Option<usize> {
        self.max_input_bytes
    }

    /// Returns the one-based parser line in normalized JSON text, when known.
    #[must_use]
    pub const fn normalized_line(&self) -> Option<usize> {
        self.normalized_line
    }

    /// Returns the one-based parser column in normalized JSON text, when known.
    #[must_use]
    pub const fn normalized_column(&self) -> Option<usize> {
        self.normalized_column
    }

    #[inline]
    pub(crate) fn input_too_large(
        raw_input_bytes: usize,
        max_input_bytes: usize,
    ) -> Self {
        Self {
            kind: JsonDecodeErrorKind::InputTooLarge,
            stage: JsonDecodeStage::Normalize,
            message: format!(
                "JSON input is too large: {raw_input_bytes} bytes exceed configured limit {max_input_bytes} bytes"
            ),
            expected_top_level: None,
            actual_top_level: None,
            raw_input_bytes,
            normalized_input_bytes: None,
            max_input_bytes: Some(max_input_bytes),
            normalized_line: None,
            normalized_column: None,
            source: None,
        }
    }

    #[inline]
    pub(crate) fn empty_input(raw_input_bytes: usize) -> Self {
        Self {
            kind: JsonDecodeErrorKind::EmptyInput,
            stage: JsonDecodeStage::Normalize,
            message: "JSON input is empty after normalization".to_string(),
            expected_top_level: None,
            actual_top_level: None,
            raw_input_bytes,
            normalized_input_bytes: None,
            max_input_bytes: None,
            normalized_line: None,
            normalized_column: None,
            source: None,
        }
    }

    #[inline]
    pub(crate) fn invalid_json(
        error: serde_json::Error,
        raw_input_bytes: usize,
        normalized_input_bytes: usize,
    ) -> Self {
        Self::from_serde_error(
            JsonDecodeErrorKind::InvalidJson,
            JsonDecodeStage::Parse,
            "Failed to parse JSON",
            error,
            raw_input_bytes,
            normalized_input_bytes,
        )
    }

    #[inline]
    pub(crate) fn unexpected_top_level(
        expected: JsonTopLevelKind,
        actual: JsonTopLevelKind,
        raw_input_bytes: usize,
        normalized_input_bytes: usize,
    ) -> Self {
        Self {
            kind: JsonDecodeErrorKind::UnexpectedTopLevel,
            stage: JsonDecodeStage::TopLevelCheck,
            message: format!(
                "Unexpected JSON top-level type: expected {expected}, got {actual}"
            ),
            expected_top_level: Some(expected),
            actual_top_level: Some(actual),
            raw_input_bytes,
            normalized_input_bytes: Some(normalized_input_bytes),
            max_input_bytes: None,
            normalized_line: None,
            normalized_column: None,
            source: None,
        }
    }

    #[inline]
    pub(crate) fn deserialize(
        error: serde_json::Error,
        raw_input_bytes: usize,
        normalized_input_bytes: usize,
    ) -> Self {
        Self::from_serde_error(
            JsonDecodeErrorKind::Deserialize,
            JsonDecodeStage::Deserialize,
            "Failed to deserialize JSON value",
            error,
            raw_input_bytes,
            normalized_input_bytes,
        )
    }

    fn from_serde_error(
        kind: JsonDecodeErrorKind,
        stage: JsonDecodeStage,
        prefix: &str,
        error: serde_json::Error,
        raw_input_bytes: usize,
        normalized_input_bytes: usize,
    ) -> Self {
        let line = error.line();
        let column = error.column();
        Self {
            kind,
            stage,
            message: format!("{prefix}: {error}"),
            expected_top_level: None,
            actual_top_level: None,
            raw_input_bytes,
            normalized_input_bytes: Some(normalized_input_bytes),
            max_input_bytes: None,
            normalized_line: (line > 0).then_some(line),
            normalized_column: (column > 0).then_some(column),
            source: Some(Arc::new(error)),
        }
    }
}

impl PartialEq for JsonDecodeError {
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind
            && self.stage == other.stage
            && self.message == other.message
            && self.expected_top_level == other.expected_top_level
            && self.actual_top_level == other.actual_top_level
            && self.raw_input_bytes == other.raw_input_bytes
            && self.normalized_input_bytes == other.normalized_input_bytes
            && self.max_input_bytes == other.max_input_bytes
            && self.normalized_line == other.normalized_line
            && self.normalized_column == other.normalized_column
    }
}

impl Eq for JsonDecodeError {}

impl fmt::Display for JsonDecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for JsonDecodeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source
            .as_deref()
            .map(|error| error as &(dyn std::error::Error + 'static))
    }
}
