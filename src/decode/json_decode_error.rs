// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines the shared error returned by all JSON decoding facades.

use std::error::Error;
use std::fmt;
use std::sync::Arc;

use qubit_budget::MeasuredBudgetError;
use qubit_budget::ResourceQuantity;
use qubit_budget::json::JsonResource;
use serde_json::Error as JsonError;

use super::DiagnosticPolicy;
use super::JsonDecodeErrorKind;
use super::JsonDecodeErrorSource;
use super::JsonDecodeStage;
use super::JsonRootKind;
use super::JsonSyntaxError;
use crate::lexical::JsonLexicalFailure;

/// Failure produced by either strict or normalizing JSON decoding.
///
/// Internal variants are private so callers branch through stable semantic
/// accessors rather than depending on scanner, normalizer, or Serde details.
///
/// # Type Parameters
///
/// * `R` - Resource identity attached to budget failures.
/// * `Q` - Quantity representation attached to budget failures.
///
/// # Examples
///
/// ```
/// use qubit_json::decode::{JsonDecodeError, JsonDecodeErrorKind, JsonDecoder};
/// use serde_json::Value;
///
/// let mut decoder = JsonDecoder::unlimited();
/// let error: JsonDecodeError = decoder
///     .decode_str::<Value>("")
///     .expect_err("empty input must be rejected");
/// assert_eq!(error.kind(), JsonDecodeErrorKind::InvalidJson);
/// ```
#[must_use]
#[derive(Clone)]
pub struct JsonDecodeError<R = JsonResource, Q = usize>
where
    Q: Copy + fmt::Debug,
{
    /// Policy controlling input-derived source retention.
    diagnostic_policy: DiagnosticPolicy,
    /// Mutually exclusive structured failure.
    failure: JsonDecodeErrorSource<R, Q>,
}

impl<R, Q> JsonDecodeError<R, Q>
where
    Q: Copy + fmt::Debug,
{
    /// Creates a measured-budget failure at a semantic stage.
    #[must_use = "return or inspect the constructed decoding error"]
    pub(crate) const fn budget(
        source: MeasuredBudgetError<R, Q>,
        stage: JsonDecodeStage,
        raw_input_bytes: usize,
        normalized_input_bytes: Option<usize>,
        diagnostic_policy: DiagnosticPolicy,
    ) -> Self {
        Self {
            diagnostic_policy,
            failure: JsonDecodeErrorSource::Budget {
                stage,
                raw_input_bytes,
                normalized_input_bytes,
                source,
            },
        }
    }

    /// Creates an empty-input failure at a semantic stage.
    #[must_use = "return or inspect the constructed decoding error"]
    pub(crate) const fn empty_input(
        stage: JsonDecodeStage,
        raw_input_bytes: usize,
        normalized_input_bytes: Option<usize>,
        diagnostic_policy: DiagnosticPolicy,
    ) -> Self {
        Self {
            diagnostic_policy,
            failure: JsonDecodeErrorSource::EmptyInput {
                stage,
                raw_input_bytes,
                normalized_input_bytes,
            },
        }
    }

    /// Creates an invalid-UTF-8 failure and conditionally retains its source.
    #[must_use = "return or inspect the constructed decoding error"]
    pub(crate) fn invalid_utf8(
        source: std::str::Utf8Error,
        raw_input_bytes: usize,
        diagnostic_policy: DiagnosticPolicy,
    ) -> Self {
        let valid_up_to = source.valid_up_to();
        let error_len = source.error_len();
        let source = (diagnostic_policy == DiagnosticPolicy::Detailed).then_some(source);
        Self {
            diagnostic_policy,
            failure: JsonDecodeErrorSource::InvalidUtf8 {
                raw_input_bytes,
                valid_up_to,
                error_len,
                source,
            },
        }
    }

    /// Creates a stable invalid-JSON failure from lexical admission.
    #[must_use = "return or inspect the constructed decoding error"]
    pub(crate) fn invalid_json(
        syntax_source: JsonLexicalFailure,
        detailed_source: Option<Arc<dyn Error + Send + Sync>>,
        raw_input_bytes: usize,
        normalized_input_bytes: Option<usize>,
        diagnostic_policy: DiagnosticPolicy,
    ) -> Self {
        Self {
            diagnostic_policy,
            failure: JsonDecodeErrorSource::InvalidJson {
                raw_input_bytes,
                normalized_input_bytes,
                syntax: JsonSyntaxError::from_lexical(syntax_source),
                source: detailed_source,
            },
        }
    }

    /// Creates an unexpected-top-level failure.
    #[must_use = "return or inspect the constructed decoding error"]
    pub(crate) const fn unexpected_top_level(
        expected: JsonRootKind,
        actual: JsonRootKind,
        raw_input_bytes: usize,
        normalized_input_bytes: Option<usize>,
        diagnostic_policy: DiagnosticPolicy,
    ) -> Self {
        Self {
            diagnostic_policy,
            failure: JsonDecodeErrorSource::UnexpectedTopLevel {
                raw_input_bytes,
                normalized_input_bytes,
                expected,
                actual,
            },
        }
    }

    /// Creates a target-deserialization failure and conditionally retains its
    /// input-derived source.
    #[must_use = "return or inspect the constructed decoding error"]
    pub(crate) fn deserialize(
        source: JsonError,
        raw_input_bytes: usize,
        normalized_input_bytes: Option<usize>,
        diagnostic_policy: DiagnosticPolicy,
    ) -> Self {
        let line = source.line();
        let column = source.column();
        let source =
            (diagnostic_policy == DiagnosticPolicy::Detailed).then(|| Arc::new(source) as Arc<dyn Error + Send + Sync>);
        Self {
            diagnostic_policy,
            failure: JsonDecodeErrorSource::Deserialize {
                raw_input_bytes,
                normalized_input_bytes,
                line,
                column,
                source,
            },
        }
    }

    /// Returns the stable failure category.
    ///
    /// # Returns
    ///
    /// The category describing which kind of decode operation failed.
    #[must_use]
    #[inline(always)]
    pub const fn kind(&self) -> JsonDecodeErrorKind {
        match self.failure {
            JsonDecodeErrorSource::Budget { .. } => JsonDecodeErrorKind::Budget,
            JsonDecodeErrorSource::EmptyInput { .. } => JsonDecodeErrorKind::EmptyInput,
            JsonDecodeErrorSource::InvalidUtf8 { .. } => JsonDecodeErrorKind::InvalidUtf8,
            JsonDecodeErrorSource::InvalidJson { .. } => JsonDecodeErrorKind::InvalidJson,
            JsonDecodeErrorSource::UnexpectedTopLevel { .. } => JsonDecodeErrorKind::UnexpectedTopLevel,
            JsonDecodeErrorSource::Deserialize { .. } => JsonDecodeErrorKind::Deserialize,
        }
    }

    /// Returns the semantic stage that produced the failure.
    ///
    /// # Returns
    ///
    /// The pipeline stage at which the failure was recorded.
    #[must_use]
    #[inline(always)]
    pub const fn stage(&self) -> JsonDecodeStage {
        match self.failure {
            JsonDecodeErrorSource::Budget { stage, .. } | JsonDecodeErrorSource::EmptyInput { stage, .. } => stage,
            JsonDecodeErrorSource::InvalidUtf8 { .. } => JsonDecodeStage::DecodeText,
            JsonDecodeErrorSource::InvalidJson { .. } => JsonDecodeStage::Parse,
            JsonDecodeErrorSource::UnexpectedTopLevel { .. } => JsonDecodeStage::TopLevelCheck,
            JsonDecodeErrorSource::Deserialize { .. } => JsonDecodeStage::Deserialize,
        }
    }

    /// Returns the diagnostic policy applied while constructing this error.
    ///
    /// # Returns
    ///
    /// The policy that determines whether input-derived details are retained.
    #[must_use]
    #[inline(always)]
    pub const fn diagnostic_policy(&self) -> DiagnosticPolicy {
        self.diagnostic_policy
    }

    /// Returns the original input length in bytes.
    ///
    /// # Returns
    ///
    /// The number of bytes charged for the input that caused this error.
    #[must_use]
    #[inline(always)]
    pub const fn raw_input_bytes(&self) -> usize {
        match self.failure {
            JsonDecodeErrorSource::Budget { raw_input_bytes, .. }
            | JsonDecodeErrorSource::EmptyInput { raw_input_bytes, .. }
            | JsonDecodeErrorSource::InvalidUtf8 { raw_input_bytes, .. }
            | JsonDecodeErrorSource::InvalidJson { raw_input_bytes, .. }
            | JsonDecodeErrorSource::UnexpectedTopLevel { raw_input_bytes, .. }
            | JsonDecodeErrorSource::Deserialize { raw_input_bytes, .. } => raw_input_bytes,
        }
    }

    /// Returns the normalized text length when normalization completed.
    ///
    /// # Returns
    ///
    /// `Some(length)` when normalization produced text, or `None` when the
    /// failure occurred before a normalized document existed.
    #[must_use]
    #[inline(always)]
    pub const fn normalized_input_bytes(&self) -> Option<usize> {
        match self.failure {
            JsonDecodeErrorSource::Budget {
                normalized_input_bytes, ..
            }
            | JsonDecodeErrorSource::EmptyInput {
                normalized_input_bytes, ..
            }
            | JsonDecodeErrorSource::InvalidJson {
                normalized_input_bytes, ..
            }
            | JsonDecodeErrorSource::UnexpectedTopLevel {
                normalized_input_bytes, ..
            }
            | JsonDecodeErrorSource::Deserialize {
                normalized_input_bytes, ..
            } => normalized_input_bytes,
            JsonDecodeErrorSource::InvalidUtf8 { .. } => None,
        }
    }

    /// Returns the one-based error line when available.
    ///
    /// # Returns
    ///
    /// `Some(line)` for failures with source coordinates, otherwise `None`.
    #[must_use]
    #[inline(always)]
    pub const fn line(&self) -> Option<usize> {
        match &self.failure {
            JsonDecodeErrorSource::InvalidJson { syntax, .. } => Some(syntax.line()),
            JsonDecodeErrorSource::Deserialize { line, .. } if *line > 0 => Some(*line),
            _ => None,
        }
    }

    /// Returns the one-based error column when available.
    ///
    /// # Returns
    ///
    /// `Some(column)` for failures with source coordinates, otherwise `None`.
    #[must_use]
    #[inline(always)]
    pub const fn column(&self) -> Option<usize> {
        match &self.failure {
            JsonDecodeErrorSource::InvalidJson { syntax, .. } => Some(syntax.column()),
            JsonDecodeErrorSource::Deserialize { column, .. } if *column > 0 => Some(*column),
            _ => None,
        }
    }

    /// Returns the structured syntax failure for invalid JSON.
    ///
    /// # Returns
    ///
    /// A borrowed syntax error when parsing failed, otherwise `None`.
    #[must_use]
    #[inline(always)]
    pub const fn syntax_error(&self) -> Option<&JsonSyntaxError> {
        match &self.failure {
            JsonDecodeErrorSource::InvalidJson { syntax, .. } => Some(syntax),
            _ => None,
        }
    }

    /// Returns the complete measured-budget failure when present.
    ///
    /// # Returns
    ///
    /// A borrowed budget error when resource accounting rejected the input,
    /// otherwise `None`.
    #[must_use]
    #[inline(always)]
    pub const fn budget_error(&self) -> Option<&MeasuredBudgetError<R, Q>> {
        match &self.failure {
            JsonDecodeErrorSource::Budget { source, .. } => Some(source),
            _ => None,
        }
    }

    /// Returns the valid UTF-8 prefix length for invalid byte input.
    ///
    /// # Returns
    ///
    /// `Some(bytes)` for an invalid UTF-8 failure, or `None` for other failure
    /// kinds.
    #[must_use]
    #[inline(always)]
    pub const fn utf8_valid_up_to(&self) -> Option<usize> {
        match self.failure {
            JsonDecodeErrorSource::InvalidUtf8 { valid_up_to, .. } => Some(valid_up_to),
            _ => None,
        }
    }

    /// Returns the invalid UTF-8 sequence length when known.
    ///
    /// # Returns
    ///
    /// The length of the invalid sequence when the decoder can determine it,
    /// otherwise `None`.
    #[must_use]
    #[inline(always)]
    pub const fn utf8_error_len(&self) -> Option<usize> {
        match self.failure {
            JsonDecodeErrorSource::InvalidUtf8 { error_len, .. } => error_len,
            _ => None,
        }
    }

    /// Returns the expected top-level kind for a constrained decode failure.
    ///
    /// # Returns
    ///
    /// The required root kind when a constrained operation failed, otherwise
    /// `None`.
    #[must_use]
    #[inline(always)]
    pub const fn expected_top_level(&self) -> Option<JsonRootKind> {
        match self.failure {
            JsonDecodeErrorSource::UnexpectedTopLevel { expected, .. } => Some(expected),
            _ => None,
        }
    }

    /// Returns the observed top-level kind for a constrained decode failure.
    ///
    /// # Returns
    ///
    /// The root kind observed in the valid document, otherwise `None`.
    #[must_use]
    #[inline(always)]
    pub const fn actual_top_level(&self) -> Option<JsonRootKind> {
        match self.failure {
            JsonDecodeErrorSource::UnexpectedTopLevel { actual, .. } => Some(actual),
            _ => None,
        }
    }

    /// Consumes this error and returns its owned semantic source.
    ///
    /// Unlike the kind-specific accessors, this operation preserves the
    /// complete mutually exclusive failure state. Input-derived third-party
    /// sources remain present only when the decoder used
    /// [`DiagnosticPolicy::Detailed`].
    ///
    /// # Returns
    ///
    /// The structured budget, input, syntax, top-level, or deserialization
    /// source retained by this error.
    #[inline(always)]
    pub fn into_source(self) -> JsonDecodeErrorSource<R, Q> {
        self.failure
    }
}

impl<R, Q> fmt::Debug for JsonDecodeError<R, Q>
where
    R: fmt::Debug,
    Q: Copy + fmt::Debug,
{
    /// Formats structured diagnostics retained under the active policy.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("JsonDecodeError")
            .field("diagnostic_policy", &self.diagnostic_policy)
            .field("failure", &self.failure)
            .finish()
    }
}

impl<R, Q> fmt::Display for JsonDecodeError<R, Q>
where
    R: fmt::Debug,
    Q: ResourceQuantity,
{
    /// Formats a privacy-safe or detailed message according to the policy.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.failure {
            JsonDecodeErrorSource::Budget { source, .. } => {
                write!(formatter, "JSON resource budget rejected input: {source}")
            }
            JsonDecodeErrorSource::EmptyInput { .. } => formatter.write_str("JSON input is empty after normalization"),
            JsonDecodeErrorSource::InvalidUtf8 { source, .. } => match source {
                Some(source) => write!(formatter, "Failed to decode JSON input as UTF-8: {source}"),
                None => formatter.write_str("Failed to decode JSON input as UTF-8"),
            },
            JsonDecodeErrorSource::InvalidJson { syntax, source, .. } => match source {
                Some(source) => {
                    write!(formatter, "Failed to parse JSON: {source}")
                }
                None => write!(formatter, "Failed to parse JSON: {syntax}"),
            },
            JsonDecodeErrorSource::UnexpectedTopLevel { expected, actual, .. } => {
                write!(
                    formatter,
                    "Unexpected JSON top-level type: expected {expected}, got {actual}"
                )
            }
            JsonDecodeErrorSource::Deserialize {
                normalized_input_bytes,
                line,
                column,
                source,
                ..
            } => match source {
                Some(source) => write!(formatter, "Failed to deserialize JSON value: {source}"),
                None if normalized_input_bytes.is_some() => write!(
                    formatter,
                    "Failed to deserialize JSON value at normalized line {line} column {column}"
                ),
                None => write!(
                    formatter,
                    "Failed to deserialize JSON value at line {line} column {column}"
                ),
            },
        }
    }
}

impl<R, Q> Error for JsonDecodeError<R, Q>
where
    R: fmt::Debug + 'static,
    Q: ResourceQuantity + 'static,
{
    /// Returns budget sources unconditionally and input-derived sources only
    /// when detailed diagnostics retained them.
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match &self.failure {
            JsonDecodeErrorSource::Budget { source, .. } => Some(source),
            JsonDecodeErrorSource::InvalidUtf8 {
                source: Some(source), ..
            } => Some(source),
            JsonDecodeErrorSource::InvalidJson {
                source: Some(source), ..
            } => Some(source.as_ref()),
            JsonDecodeErrorSource::Deserialize {
                source: Some(source), ..
            } => Some(source.as_ref()),
            _ => None,
        }
    }
}
