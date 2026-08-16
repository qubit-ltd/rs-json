// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines the [`LenientJsonDecodeError`] type used by the lenient decoder API.

use std::error::Error;
use std::fmt;
use std::sync::Arc;

use qubit_budget::MeasuredBudgetError;
use qubit_budget::json::JsonResource;

use super::ErrorPrivacyPolicy;
use super::JsonTopLevelKind;
use super::LenientJsonDecodeErrorKind;
use super::LenientJsonDecodeStage;
use crate::internal::JsonLexicalFailure;

/// Error returned when lenient JSON decoding fails.
///
/// Each private failure variant owns exactly the diagnostics that are valid for
/// that failure, so public accessors cannot expose contradictory state.
#[non_exhaustive]
#[derive(Clone)]
pub struct LenientJsonDecodeError {
    /// Stores the privacy policy applied while constructing diagnostics.
    privacy_policy: ErrorPrivacyPolicy,
    /// Stores the structured failure and its variant-specific diagnostics.
    failure: LenientJsonDecodeFailure,
}

/// Stores the mutually exclusive states of a lenient decoding failure.
#[derive(Debug)]
enum LenientJsonDecodeFailure {
    /// Raw input was not valid UTF-8.
    InvalidUtf8 {
        raw_input_bytes: usize,
        valid_up_to: usize,
        error_len: Option<usize>,
        source: Option<std::str::Utf8Error>,
    },
    /// Raw input exceeded its configured byte limit.
    InputTooLarge {
        raw_input_bytes: usize,
        maximum: usize,
    },
    /// Normalized input exceeded its configured byte limit.
    NormalizedInputTooLarge {
        raw_input_bytes: usize,
        normalized_input_bytes: usize,
        maximum: usize,
    },
    /// Input was empty at a normalization boundary.
    EmptyInput {
        raw_input_bytes: usize,
        normalized_input_bytes: Option<usize>,
    },
    /// Normalized text was not valid JSON.
    InvalidJson {
        raw_input_bytes: usize,
        normalized_input_bytes: usize,
        line: usize,
        column: usize,
        source: Option<Arc<dyn Error + Send + Sync>>,
    },
    /// Decoded JSON exceeded a resource budget.
    Budget {
        raw_input_bytes: usize,
        normalized_input_bytes: Option<usize>,
        source: MeasuredBudgetError<JsonResource, usize>,
    },
    /// Valid JSON had an unexpected top-level kind.
    UnexpectedTopLevel {
        raw_input_bytes: usize,
        normalized_input_bytes: usize,
        expected: JsonTopLevelKind,
        actual: JsonTopLevelKind,
    },
    /// Valid JSON could not deserialize into the requested type.
    Deserialize {
        raw_input_bytes: usize,
        normalized_input_bytes: usize,
        line: usize,
        column: usize,
        source: Option<Arc<dyn Error + Send + Sync>>,
    },
}

impl Clone for LenientJsonDecodeFailure {
    /// Clones structured diagnostics, including the reconstructible budget
    /// source and any retained reference-counted input source.
    fn clone(&self) -> Self {
        match self {
            Self::InvalidUtf8 {
                raw_input_bytes,
                valid_up_to,
                error_len,
                source,
            } => Self::InvalidUtf8 {
                raw_input_bytes: *raw_input_bytes,
                valid_up_to: *valid_up_to,
                error_len: *error_len,
                source: *source,
            },
            Self::InputTooLarge {
                raw_input_bytes,
                maximum,
            } => Self::InputTooLarge {
                raw_input_bytes: *raw_input_bytes,
                maximum: *maximum,
            },
            Self::NormalizedInputTooLarge {
                raw_input_bytes,
                normalized_input_bytes,
                maximum,
            } => Self::NormalizedInputTooLarge {
                raw_input_bytes: *raw_input_bytes,
                normalized_input_bytes: *normalized_input_bytes,
                maximum: *maximum,
            },
            Self::EmptyInput {
                raw_input_bytes,
                normalized_input_bytes,
            } => Self::EmptyInput {
                raw_input_bytes: *raw_input_bytes,
                normalized_input_bytes: *normalized_input_bytes,
            },
            Self::InvalidJson {
                raw_input_bytes,
                normalized_input_bytes,
                line,
                column,
                source,
            } => Self::InvalidJson {
                raw_input_bytes: *raw_input_bytes,
                normalized_input_bytes: *normalized_input_bytes,
                line: *line,
                column: *column,
                source: source.clone(),
            },
            Self::Budget {
                raw_input_bytes,
                normalized_input_bytes,
                source,
            } => Self::Budget {
                raw_input_bytes: *raw_input_bytes,
                normalized_input_bytes: *normalized_input_bytes,
                source: match source {
                    MeasuredBudgetError::Quantity { resource, source } => {
                        MeasuredBudgetError::quantity(*resource, *source)
                    }
                    MeasuredBudgetError::Budget(error) => {
                        MeasuredBudgetError::Budget(error.clone())
                    }
                },
            },
            Self::UnexpectedTopLevel {
                raw_input_bytes,
                normalized_input_bytes,
                expected,
                actual,
            } => Self::UnexpectedTopLevel {
                raw_input_bytes: *raw_input_bytes,
                normalized_input_bytes: *normalized_input_bytes,
                expected: *expected,
                actual: *actual,
            },
            Self::Deserialize {
                raw_input_bytes,
                normalized_input_bytes,
                line,
                column,
                source,
            } => Self::Deserialize {
                raw_input_bytes: *raw_input_bytes,
                normalized_input_bytes: *normalized_input_bytes,
                line: *line,
                column: *column,
                source: source.clone(),
            },
        }
    }
}

impl LenientJsonDecodeError {
    /// Creates an error for raw bytes that are not valid UTF-8.
    ///
    /// The safe byte offsets are retained under both privacy policies. The
    /// source is retained only in detailed mode.
    #[must_use]
    pub(crate) fn invalid_utf8(
        error: std::str::Utf8Error,
        raw_input_bytes: usize,
        privacy_policy: ErrorPrivacyPolicy,
    ) -> Self {
        let valid_up_to = error.valid_up_to();
        let error_len = error.error_len();
        let source =
            (privacy_policy == ErrorPrivacyPolicy::Detailed).then_some(error);
        Self {
            privacy_policy,
            failure: LenientJsonDecodeFailure::InvalidUtf8 {
                raw_input_bytes,
                valid_up_to,
                error_len,
                source,
            },
        }
    }

    /// Creates an error for raw input that exceeds the configured size limit.
    #[must_use]
    pub(crate) const fn input_too_large(
        raw_input_bytes: usize,
        maximum: usize,
        privacy_policy: ErrorPrivacyPolicy,
    ) -> Self {
        Self {
            privacy_policy,
            failure: LenientJsonDecodeFailure::InputTooLarge {
                raw_input_bytes,
                maximum,
            },
        }
    }

    /// Creates an error for normalized input that exceeds its configured
    /// limit.
    #[must_use]
    pub(crate) const fn normalized_input_too_large(
        raw_input_bytes: usize,
        normalized_input_bytes: usize,
        maximum: usize,
        privacy_policy: ErrorPrivacyPolicy,
    ) -> Self {
        Self {
            privacy_policy,
            failure: LenientJsonDecodeFailure::NormalizedInputTooLarge {
                raw_input_bytes,
                normalized_input_bytes,
                maximum,
            },
        }
    }

    /// Creates an error for input that is empty at a normalization boundary.
    #[must_use]
    pub(crate) const fn empty_input(
        raw_input_bytes: usize,
        normalized_input_bytes: Option<usize>,
        privacy_policy: ErrorPrivacyPolicy,
    ) -> Self {
        Self {
            privacy_policy,
            failure: LenientJsonDecodeFailure::EmptyInput {
                raw_input_bytes,
                normalized_input_bytes,
            },
        }
    }

    /// Creates an error for invalid normalized JSON syntax.
    #[must_use]
    pub(crate) fn invalid_json(
        error: serde_json::Error,
        raw_input_bytes: usize,
        normalized_input_bytes: usize,
        privacy_policy: ErrorPrivacyPolicy,
    ) -> Self {
        let line = error.line();
        let column = error.column();
        let source = Self::retain_input_source(error, privacy_policy);
        Self {
            privacy_policy,
            failure: LenientJsonDecodeFailure::InvalidJson {
                raw_input_bytes,
                normalized_input_bytes,
                line,
                column,
                source,
            },
        }
    }

    /// Creates an invalid-JSON error from non-recursive lexical admission.
    #[must_use]
    pub(crate) fn invalid_lexical_json(
        error: JsonLexicalFailure,
        raw_input_bytes: usize,
        normalized_input_bytes: usize,
        privacy_policy: ErrorPrivacyPolicy,
    ) -> Self {
        let line = error.line;
        let column = error.column;
        let source = Self::retain_input_source(error, privacy_policy);
        Self {
            privacy_policy,
            failure: LenientJsonDecodeFailure::InvalidJson {
                raw_input_bytes,
                normalized_input_bytes,
                line,
                column,
                source,
            },
        }
    }

    /// Creates an error for a decoded JSON value budget rejection.
    #[must_use]
    pub(crate) const fn budget(
        source: MeasuredBudgetError<JsonResource, usize>,
        raw_input_bytes: usize,
        normalized_input_bytes: usize,
        privacy_policy: ErrorPrivacyPolicy,
    ) -> Self {
        Self {
            privacy_policy,
            failure: LenientJsonDecodeFailure::Budget {
                raw_input_bytes,
                normalized_input_bytes: Some(normalized_input_bytes),
                source,
            },
        }
    }

    /// Creates an error for a valid JSON value with an unexpected top-level
    /// kind.
    #[must_use]
    pub(crate) const fn unexpected_top_level(
        expected: JsonTopLevelKind,
        actual: JsonTopLevelKind,
        raw_input_bytes: usize,
        normalized_input_bytes: usize,
        privacy_policy: ErrorPrivacyPolicy,
    ) -> Self {
        Self {
            privacy_policy,
            failure: LenientJsonDecodeFailure::UnexpectedTopLevel {
                raw_input_bytes,
                normalized_input_bytes,
                expected,
                actual,
            },
        }
    }

    /// Creates an error for valid JSON that cannot deserialize into the target.
    #[must_use]
    pub(crate) fn deserialize(
        error: serde_json::Error,
        raw_input_bytes: usize,
        normalized_input_bytes: usize,
        privacy_policy: ErrorPrivacyPolicy,
    ) -> Self {
        let line = error.line();
        let column = error.column();
        let source = Self::retain_input_source(error, privacy_policy);
        Self {
            privacy_policy,
            failure: LenientJsonDecodeFailure::Deserialize {
                raw_input_bytes,
                normalized_input_bytes,
                line,
                column,
                source,
            },
        }
    }

    /// Retains an input-derived source only when detailed diagnostics are
    /// enabled.
    fn retain_input_source<E>(
        error: E,
        privacy_policy: ErrorPrivacyPolicy,
    ) -> Option<Arc<dyn Error + Send + Sync>>
    where
        E: Error + Send + Sync + 'static,
    {
        (privacy_policy == ErrorPrivacyPolicy::Detailed)
            .then(|| Arc::new(error) as Arc<dyn Error + Send + Sync>)
    }

    /// Returns the stable category of this decoding failure.
    ///
    /// # Returns
    ///
    /// The stable category describing the failure.
    #[must_use]
    #[inline(always)]
    pub const fn kind(&self) -> LenientJsonDecodeErrorKind {
        match self.failure {
            LenientJsonDecodeFailure::InvalidUtf8 { .. } => {
                LenientJsonDecodeErrorKind::InvalidUtf8
            }
            LenientJsonDecodeFailure::InputTooLarge { .. }
            | LenientJsonDecodeFailure::NormalizedInputTooLarge { .. } => {
                LenientJsonDecodeErrorKind::InputTooLarge
            }
            LenientJsonDecodeFailure::EmptyInput { .. } => {
                LenientJsonDecodeErrorKind::EmptyInput
            }
            LenientJsonDecodeFailure::InvalidJson { .. } => {
                LenientJsonDecodeErrorKind::InvalidJson
            }
            LenientJsonDecodeFailure::Budget { .. } => {
                LenientJsonDecodeErrorKind::Budget
            }
            LenientJsonDecodeFailure::UnexpectedTopLevel { .. } => {
                LenientJsonDecodeErrorKind::UnexpectedTopLevel
            }
            LenientJsonDecodeFailure::Deserialize { .. } => {
                LenientJsonDecodeErrorKind::Deserialize
            }
        }
    }

    /// Returns the decoding stage that produced this error.
    ///
    /// # Returns
    ///
    /// The normalization, parsing, admission, top-level-check, or
    /// deserialization stage that produced the error.
    #[must_use]
    #[inline(always)]
    pub const fn stage(&self) -> LenientJsonDecodeStage {
        match self.failure {
            LenientJsonDecodeFailure::InvalidUtf8 { .. } => {
                LenientJsonDecodeStage::DecodeText
            }
            LenientJsonDecodeFailure::InputTooLarge { .. }
            | LenientJsonDecodeFailure::NormalizedInputTooLarge { .. }
            | LenientJsonDecodeFailure::EmptyInput { .. } => {
                LenientJsonDecodeStage::Normalize
            }
            LenientJsonDecodeFailure::InvalidJson { .. } => {
                LenientJsonDecodeStage::Parse
            }
            LenientJsonDecodeFailure::Budget { .. } => {
                LenientJsonDecodeStage::Admission
            }
            LenientJsonDecodeFailure::UnexpectedTopLevel { .. } => {
                LenientJsonDecodeStage::TopLevelCheck
            }
            LenientJsonDecodeFailure::Deserialize { .. } => {
                LenientJsonDecodeStage::Deserialize
            }
        }
    }

    /// Returns the privacy policy applied when this error was constructed.
    ///
    /// # Returns
    ///
    /// The policy controlling retention of input-derived diagnostics.
    #[must_use]
    #[inline(always)]
    pub const fn privacy_policy(&self) -> ErrorPrivacyPolicy {
        self.privacy_policy
    }

    /// Returns the required top-level JSON kind for a constrained decode.
    ///
    /// # Returns
    ///
    /// `Some(kind)` for an unexpected-top-level failure, or `None` for other
    /// failure categories.
    #[must_use]
    #[inline(always)]
    pub const fn expected_top_level(&self) -> Option<JsonTopLevelKind> {
        match self.failure {
            LenientJsonDecodeFailure::UnexpectedTopLevel {
                expected, ..
            } => Some(expected),
            _ => None,
        }
    }

    /// Returns the observed top-level JSON kind for a constrained decode.
    ///
    /// # Returns
    ///
    /// `Some(kind)` for an unexpected-top-level failure, or `None` for other
    /// failure categories.
    #[must_use]
    #[inline(always)]
    pub const fn actual_top_level(&self) -> Option<JsonTopLevelKind> {
        match self.failure {
            LenientJsonDecodeFailure::UnexpectedTopLevel { actual, .. } => {
                Some(actual)
            }
            _ => None,
        }
    }

    /// Returns the byte length of the input before normalization.
    ///
    /// # Returns
    ///
    /// The raw input length in bytes.
    #[must_use]
    #[inline(always)]
    pub const fn raw_input_bytes(&self) -> usize {
        match self.failure {
            LenientJsonDecodeFailure::InvalidUtf8 {
                raw_input_bytes, ..
            }
            | LenientJsonDecodeFailure::InputTooLarge {
                raw_input_bytes, ..
            }
            | LenientJsonDecodeFailure::NormalizedInputTooLarge {
                raw_input_bytes,
                ..
            }
            | LenientJsonDecodeFailure::EmptyInput {
                raw_input_bytes, ..
            }
            | LenientJsonDecodeFailure::InvalidJson {
                raw_input_bytes, ..
            }
            | LenientJsonDecodeFailure::Budget {
                raw_input_bytes, ..
            }
            | LenientJsonDecodeFailure::UnexpectedTopLevel {
                raw_input_bytes,
                ..
            }
            | LenientJsonDecodeFailure::Deserialize {
                raw_input_bytes, ..
            } => raw_input_bytes,
        }
    }

    /// Returns the valid UTF-8 prefix length for invalid byte input.
    ///
    /// # Returns
    ///
    /// `Some(bytes)` for an invalid-UTF-8 failure, or `None` otherwise.
    #[must_use]
    #[inline(always)]
    pub const fn utf8_valid_up_to(&self) -> Option<usize> {
        match self.failure {
            LenientJsonDecodeFailure::InvalidUtf8 { valid_up_to, .. } => {
                Some(valid_up_to)
            }
            _ => None,
        }
    }

    /// Returns the known length of the invalid UTF-8 sequence.
    ///
    /// # Returns
    ///
    /// The invalid sequence length when reported by UTF-8 validation, or
    /// `None` when its length is unknown or the failure is another kind.
    #[must_use]
    #[inline(always)]
    pub const fn utf8_error_len(&self) -> Option<usize> {
        match self.failure {
            LenientJsonDecodeFailure::InvalidUtf8 { error_len, .. } => {
                error_len
            }
            _ => None,
        }
    }

    /// Returns the byte length of normalized JSON text when available.
    ///
    /// # Returns
    ///
    /// `Some(bytes)` when normalization produced a measurable length, or
    /// `None` when no normalized text exists.
    #[must_use]
    #[inline(always)]
    pub const fn normalized_input_bytes(&self) -> Option<usize> {
        match self.failure {
            LenientJsonDecodeFailure::InvalidUtf8 { .. }
            | LenientJsonDecodeFailure::InputTooLarge { .. } => None,
            LenientJsonDecodeFailure::NormalizedInputTooLarge {
                normalized_input_bytes,
                ..
            }
            | LenientJsonDecodeFailure::InvalidJson {
                normalized_input_bytes,
                ..
            }
            | LenientJsonDecodeFailure::UnexpectedTopLevel {
                normalized_input_bytes,
                ..
            }
            | LenientJsonDecodeFailure::Deserialize {
                normalized_input_bytes,
                ..
            } => Some(normalized_input_bytes),
            LenientJsonDecodeFailure::EmptyInput {
                normalized_input_bytes,
                ..
            }
            | LenientJsonDecodeFailure::Budget {
                normalized_input_bytes,
                ..
            } => normalized_input_bytes,
        }
    }

    /// Returns the configured raw-input limit for a raw-size failure.
    ///
    /// # Returns
    ///
    /// `Some(limit)` for an input-size failure, or `None` otherwise.
    #[must_use]
    #[inline(always)]
    pub const fn max_input_bytes(&self) -> Option<usize> {
        match self.failure {
            LenientJsonDecodeFailure::InputTooLarge { maximum, .. } => {
                Some(maximum)
            }
            _ => None,
        }
    }

    /// Returns the configured normalized-input limit for a normalized-size
    /// failure.
    ///
    /// # Returns
    ///
    /// `Some(limit)` for a normalized-size failure, or `None` otherwise.
    #[must_use]
    #[inline(always)]
    pub const fn max_normalized_bytes(&self) -> Option<usize> {
        match self.failure {
            LenientJsonDecodeFailure::NormalizedInputTooLarge {
                maximum,
                ..
            } => Some(maximum),
            _ => None,
        }
    }

    /// Returns the parser line in normalized JSON text when available.
    ///
    /// # Returns
    ///
    /// `Some(line)` when the failure includes a positive parser line, or
    /// `None` otherwise.
    #[must_use]
    #[inline(always)]
    pub const fn normalized_line(&self) -> Option<usize> {
        match self.failure {
            LenientJsonDecodeFailure::InvalidJson { line, .. }
            | LenientJsonDecodeFailure::Deserialize { line, .. }
                if line > 0 =>
            {
                Some(line)
            }
            _ => None,
        }
    }

    /// Returns the parser column in normalized JSON text when available.
    ///
    /// # Returns
    ///
    /// `Some(column)` when the failure includes a positive parser column, or
    /// `None` otherwise.
    #[must_use]
    #[inline(always)]
    pub const fn normalized_column(&self) -> Option<usize> {
        match self.failure {
            LenientJsonDecodeFailure::InvalidJson { column, .. }
            | LenientJsonDecodeFailure::Deserialize { column, .. }
                if column > 0 =>
            {
                Some(column)
            }
            _ => None,
        }
    }

    /// Returns the complete decoded-value budget rejection when present.
    ///
    /// # Returns
    ///
    /// `Some(error)` for a decoded-value budget failure, or `None` for other
    /// failure categories.
    #[must_use]
    #[inline(always)]
    pub const fn measured_budget_error(
        &self,
    ) -> Option<&MeasuredBudgetError<JsonResource, usize>> {
        match &self.failure {
            LenientJsonDecodeFailure::Budget { source, .. } => Some(source),
            _ => None,
        }
    }

    /// Formats a stable input-derived diagnostic with safe parser location.
    fn fmt_input_failure(
        f: &mut fmt::Formatter<'_>,
        prefix: &str,
        line: usize,
        column: usize,
        source: Option<&Arc<dyn Error + Send + Sync>>,
    ) -> fmt::Result {
        if let Some(source) = source {
            write!(f, "{prefix}: {source}")
        } else {
            write!(f, "{prefix}")?;
            if line > 0 {
                write!(f, " at normalized line {line}")?;
            }
            if column > 0 {
                write!(f, " column {column}")?;
            }
            Ok(())
        }
    }
}

impl PartialEq for LenientJsonDecodeFailure {
    /// Compares structured diagnostics while ignoring input-derived sources.
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Self::InvalidUtf8 {
                    raw_input_bytes: left_raw,
                    valid_up_to: left_valid,
                    error_len: left_len,
                    ..
                },
                Self::InvalidUtf8 {
                    raw_input_bytes: right_raw,
                    valid_up_to: right_valid,
                    error_len: right_len,
                    ..
                },
            ) => {
                (left_raw, left_valid, left_len)
                    == (right_raw, right_valid, right_len)
            }
            (
                Self::InputTooLarge {
                    raw_input_bytes: left_raw,
                    maximum: left_maximum,
                },
                Self::InputTooLarge {
                    raw_input_bytes: right_raw,
                    maximum: right_maximum,
                },
            ) => (left_raw, left_maximum) == (right_raw, right_maximum),
            (
                Self::NormalizedInputTooLarge {
                    raw_input_bytes: left_raw,
                    normalized_input_bytes: left_normalized,
                    maximum: left_maximum,
                },
                Self::NormalizedInputTooLarge {
                    raw_input_bytes: right_raw,
                    normalized_input_bytes: right_normalized,
                    maximum: right_maximum,
                },
            ) => {
                (left_raw, left_normalized, left_maximum)
                    == (right_raw, right_normalized, right_maximum)
            }
            (
                Self::EmptyInput {
                    raw_input_bytes: left_raw,
                    normalized_input_bytes: left_normalized,
                },
                Self::EmptyInput {
                    raw_input_bytes: right_raw,
                    normalized_input_bytes: right_normalized,
                },
            ) => (left_raw, left_normalized) == (right_raw, right_normalized),
            (
                Self::InvalidJson {
                    raw_input_bytes: left_raw,
                    normalized_input_bytes: left_normalized,
                    line: left_line,
                    column: left_column,
                    ..
                },
                Self::InvalidJson {
                    raw_input_bytes: right_raw,
                    normalized_input_bytes: right_normalized,
                    line: right_line,
                    column: right_column,
                    ..
                },
            )
            | (
                Self::Deserialize {
                    raw_input_bytes: left_raw,
                    normalized_input_bytes: left_normalized,
                    line: left_line,
                    column: left_column,
                    ..
                },
                Self::Deserialize {
                    raw_input_bytes: right_raw,
                    normalized_input_bytes: right_normalized,
                    line: right_line,
                    column: right_column,
                    ..
                },
            ) => {
                (left_raw, left_normalized, left_line, left_column)
                    == (right_raw, right_normalized, right_line, right_column)
            }
            (
                Self::Budget {
                    raw_input_bytes: left_raw,
                    normalized_input_bytes: left_normalized,
                    source: left_source,
                },
                Self::Budget {
                    raw_input_bytes: right_raw,
                    normalized_input_bytes: right_normalized,
                    source: right_source,
                },
            ) => {
                (left_raw, left_normalized, left_source.resource())
                    == (right_raw, right_normalized, right_source.resource())
            }
            (
                Self::UnexpectedTopLevel {
                    raw_input_bytes: left_raw,
                    normalized_input_bytes: left_normalized,
                    expected: left_expected,
                    actual: left_actual,
                },
                Self::UnexpectedTopLevel {
                    raw_input_bytes: right_raw,
                    normalized_input_bytes: right_normalized,
                    expected: right_expected,
                    actual: right_actual,
                },
            ) => {
                (left_raw, left_normalized, left_expected, left_actual)
                    == (
                        right_raw,
                        right_normalized,
                        right_expected,
                        right_actual,
                    )
            }
            _ => false,
        }
    }
}

impl Eq for LenientJsonDecodeFailure {}

impl PartialEq for LenientJsonDecodeError {
    /// Compares the privacy policy and all structured diagnostics.
    fn eq(&self, other: &Self) -> bool {
        self.privacy_policy == other.privacy_policy
            && self.failure == other.failure
    }
}

impl Eq for LenientJsonDecodeError {}

impl fmt::Debug for LenientJsonDecodeError {
    /// Formats structured diagnostics without reconstructing redacted sources.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LenientJsonDecodeError")
            .field("privacy_policy", &self.privacy_policy)
            .field("failure", &self.failure)
            .finish()
    }
}

impl fmt::Display for LenientJsonDecodeFailure {
    /// Formats the message implied directly by this failure state.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidUtf8 { source, .. } => match source {
                Some(source) => {
                    write!(f, "Failed to decode JSON input as UTF-8: {source}")
                }
                None => f.write_str("Failed to decode JSON input as UTF-8"),
            },
            Self::InputTooLarge {
                raw_input_bytes,
                maximum,
            } => write!(
                f,
                "JSON input is too large: {raw_input_bytes} bytes exceed configured limit {maximum} bytes"
            ),
            Self::NormalizedInputTooLarge {
                normalized_input_bytes,
                maximum,
                ..
            } => write!(
                f,
                "Normalized JSON input is too large: {normalized_input_bytes} bytes exceed configured limit {maximum} bytes"
            ),
            Self::EmptyInput { .. } => {
                f.write_str("JSON input is empty after normalization")
            }
            Self::InvalidJson {
                line,
                column,
                source,
                ..
            } => LenientJsonDecodeError::fmt_input_failure(
                f,
                "Failed to parse JSON",
                *line,
                *column,
                source.as_ref(),
            ),
            Self::Budget { source, .. } => {
                write!(f, "JSON resource budget rejected input: {source}")
            }
            Self::UnexpectedTopLevel {
                expected, actual, ..
            } => write!(
                f,
                "Unexpected JSON top-level type: expected {expected}, got {actual}"
            ),
            Self::Deserialize {
                line,
                column,
                source,
                ..
            } => LenientJsonDecodeError::fmt_input_failure(
                f,
                "Failed to deserialize JSON value",
                *line,
                *column,
                source.as_ref(),
            ),
        }
    }
}

impl fmt::Display for LenientJsonDecodeError {
    /// Formats the message implied by the structured failure state.
    #[inline(always)]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.failure.fmt(f)
    }
}

impl Error for LenientJsonDecodeError {
    /// Returns budget sources unconditionally and input-derived sources only
    /// when detailed diagnostics retained them.
    #[inline(always)]
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match &self.failure {
            LenientJsonDecodeFailure::InvalidUtf8 {
                source: Some(source),
                ..
            } => Some(source),
            LenientJsonDecodeFailure::InvalidJson {
                source: Some(source),
                ..
            }
            | LenientJsonDecodeFailure::Deserialize {
                source: Some(source),
                ..
            } => Some(source.as_ref()),
            LenientJsonDecodeFailure::Budget { source, .. } => Some(source),
            _ => None,
        }
    }
}
