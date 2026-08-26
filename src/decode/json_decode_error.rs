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
use super::JsonDecodeStage;
use super::JsonRootKind;
use super::JsonSyntaxError;
use super::internal::JsonDecodeFailure;
use crate::lexical::JsonLexicalFailure;

/// Failure produced by either strict or normalizing JSON decoding.
///
/// Internal variants are private so callers branch through stable semantic
/// accessors rather than depending on scanner, normalizer, or Serde details.
#[derive(Clone)]
pub struct JsonDecodeError<R = JsonResource, Q = usize>
where
    Q: Copy + fmt::Debug,
{
    /// Policy controlling input-derived source retention.
    diagnostic_policy: DiagnosticPolicy,
    /// Mutually exclusive structured failure.
    failure: JsonDecodeFailure<R, Q>,
}

impl<R, Q> JsonDecodeError<R, Q>
where
    Q: Copy + fmt::Debug,
{
    /// Creates a measured-budget failure at a semantic stage.
    #[must_use]
    pub(crate) const fn budget(
        source: MeasuredBudgetError<R, Q>,
        stage: JsonDecodeStage,
        raw_input_bytes: usize,
        normalized_input_bytes: Option<usize>,
        diagnostic_policy: DiagnosticPolicy,
    ) -> Self {
        Self {
            diagnostic_policy,
            failure: JsonDecodeFailure::Budget {
                stage,
                raw_input_bytes,
                normalized_input_bytes,
                source,
            },
        }
    }

    /// Creates an empty-input failure at a semantic stage.
    #[must_use]
    pub(crate) const fn empty_input(
        stage: JsonDecodeStage,
        raw_input_bytes: usize,
        normalized_input_bytes: Option<usize>,
        diagnostic_policy: DiagnosticPolicy,
    ) -> Self {
        Self {
            diagnostic_policy,
            failure: JsonDecodeFailure::EmptyInput {
                stage,
                raw_input_bytes,
                normalized_input_bytes,
            },
        }
    }

    /// Creates an invalid-UTF-8 failure and conditionally retains its source.
    #[must_use]
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
            failure: JsonDecodeFailure::InvalidUtf8 {
                raw_input_bytes,
                valid_up_to,
                error_len,
                source,
            },
        }
    }

    /// Creates a stable invalid-JSON failure from lexical admission.
    #[must_use]
    pub(crate) fn invalid_json(
        syntax_source: JsonLexicalFailure,
        detailed_source: Option<Arc<dyn Error + Send + Sync>>,
        raw_input_bytes: usize,
        normalized_input_bytes: Option<usize>,
        diagnostic_policy: DiagnosticPolicy,
    ) -> Self {
        Self {
            diagnostic_policy,
            failure: JsonDecodeFailure::InvalidJson {
                raw_input_bytes,
                normalized_input_bytes,
                syntax: JsonSyntaxError::from_lexical(syntax_source),
                source: detailed_source,
            },
        }
    }

    /// Creates an unexpected-top-level failure.
    #[must_use]
    pub(crate) const fn unexpected_top_level(
        expected: JsonRootKind,
        actual: JsonRootKind,
        raw_input_bytes: usize,
        normalized_input_bytes: Option<usize>,
        diagnostic_policy: DiagnosticPolicy,
    ) -> Self {
        Self {
            diagnostic_policy,
            failure: JsonDecodeFailure::UnexpectedTopLevel {
                raw_input_bytes,
                normalized_input_bytes,
                expected,
                actual,
            },
        }
    }

    /// Creates a target-deserialization failure and conditionally retains its
    /// input-derived source.
    #[must_use]
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
            failure: JsonDecodeFailure::Deserialize {
                raw_input_bytes,
                normalized_input_bytes,
                line,
                column,
                source,
            },
        }
    }

    /// Returns the stable failure category.
    #[must_use]
    pub const fn kind(&self) -> JsonDecodeErrorKind {
        match self.failure {
            JsonDecodeFailure::Budget { .. } => JsonDecodeErrorKind::Budget,
            JsonDecodeFailure::EmptyInput { .. } => JsonDecodeErrorKind::EmptyInput,
            JsonDecodeFailure::InvalidUtf8 { .. } => JsonDecodeErrorKind::InvalidUtf8,
            JsonDecodeFailure::InvalidJson { .. } => JsonDecodeErrorKind::InvalidJson,
            JsonDecodeFailure::UnexpectedTopLevel { .. } => JsonDecodeErrorKind::UnexpectedTopLevel,
            JsonDecodeFailure::Deserialize { .. } => JsonDecodeErrorKind::Deserialize,
        }
    }

    /// Returns the semantic stage that produced the failure.
    #[must_use]
    pub const fn stage(&self) -> JsonDecodeStage {
        match self.failure {
            JsonDecodeFailure::Budget { stage, .. } | JsonDecodeFailure::EmptyInput { stage, .. } => stage,
            JsonDecodeFailure::InvalidUtf8 { .. } => JsonDecodeStage::DecodeText,
            JsonDecodeFailure::InvalidJson { .. } => JsonDecodeStage::Parse,
            JsonDecodeFailure::UnexpectedTopLevel { .. } => JsonDecodeStage::TopLevelCheck,
            JsonDecodeFailure::Deserialize { .. } => JsonDecodeStage::Deserialize,
        }
    }

    /// Returns the diagnostic policy applied while constructing this error.
    #[must_use]
    pub const fn diagnostic_policy(&self) -> DiagnosticPolicy {
        self.diagnostic_policy
    }

    /// Returns the original input length in bytes.
    #[must_use]
    pub const fn raw_input_bytes(&self) -> usize {
        match self.failure {
            JsonDecodeFailure::Budget { raw_input_bytes, .. }
            | JsonDecodeFailure::EmptyInput { raw_input_bytes, .. }
            | JsonDecodeFailure::InvalidUtf8 { raw_input_bytes, .. }
            | JsonDecodeFailure::InvalidJson { raw_input_bytes, .. }
            | JsonDecodeFailure::UnexpectedTopLevel { raw_input_bytes, .. }
            | JsonDecodeFailure::Deserialize { raw_input_bytes, .. } => raw_input_bytes,
        }
    }

    /// Returns the normalized text length when normalization completed.
    #[must_use]
    pub const fn normalized_input_bytes(&self) -> Option<usize> {
        match self.failure {
            JsonDecodeFailure::Budget {
                normalized_input_bytes, ..
            }
            | JsonDecodeFailure::EmptyInput {
                normalized_input_bytes, ..
            }
            | JsonDecodeFailure::InvalidJson {
                normalized_input_bytes, ..
            }
            | JsonDecodeFailure::UnexpectedTopLevel {
                normalized_input_bytes, ..
            }
            | JsonDecodeFailure::Deserialize {
                normalized_input_bytes, ..
            } => normalized_input_bytes,
            JsonDecodeFailure::InvalidUtf8 { .. } => None,
        }
    }

    /// Returns the one-based error line when available.
    #[must_use]
    pub const fn line(&self) -> Option<usize> {
        match &self.failure {
            JsonDecodeFailure::InvalidJson { syntax, .. } => Some(syntax.line()),
            JsonDecodeFailure::Deserialize { line, .. } if *line > 0 => Some(*line),
            _ => None,
        }
    }

    /// Returns the one-based error column when available.
    #[must_use]
    pub const fn column(&self) -> Option<usize> {
        match &self.failure {
            JsonDecodeFailure::InvalidJson { syntax, .. } => Some(syntax.column()),
            JsonDecodeFailure::Deserialize { column, .. } if *column > 0 => Some(*column),
            _ => None,
        }
    }

    /// Returns the structured syntax failure for invalid JSON.
    #[must_use]
    pub const fn syntax_error(&self) -> Option<&JsonSyntaxError> {
        match &self.failure {
            JsonDecodeFailure::InvalidJson { syntax, .. } => Some(syntax),
            _ => None,
        }
    }

    /// Returns the complete measured-budget failure when present.
    #[must_use]
    pub const fn budget_error(&self) -> Option<&MeasuredBudgetError<R, Q>> {
        match &self.failure {
            JsonDecodeFailure::Budget { source, .. } => Some(source),
            _ => None,
        }
    }

    /// Returns the valid UTF-8 prefix length for invalid byte input.
    #[must_use]
    pub const fn utf8_valid_up_to(&self) -> Option<usize> {
        match self.failure {
            JsonDecodeFailure::InvalidUtf8 { valid_up_to, .. } => Some(valid_up_to),
            _ => None,
        }
    }

    /// Returns the invalid UTF-8 sequence length when known.
    #[must_use]
    pub const fn utf8_error_len(&self) -> Option<usize> {
        match self.failure {
            JsonDecodeFailure::InvalidUtf8 { error_len, .. } => error_len,
            _ => None,
        }
    }

    /// Returns the expected top-level kind for a constrained decode failure.
    #[must_use]
    pub const fn expected_top_level(&self) -> Option<JsonRootKind> {
        match self.failure {
            JsonDecodeFailure::UnexpectedTopLevel { expected, .. } => Some(expected),
            _ => None,
        }
    }

    /// Returns the observed top-level kind for a constrained decode failure.
    #[must_use]
    pub const fn actual_top_level(&self) -> Option<JsonRootKind> {
        match self.failure {
            JsonDecodeFailure::UnexpectedTopLevel { actual, .. } => Some(actual),
            _ => None,
        }
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
            JsonDecodeFailure::Budget { source, .. } => {
                write!(formatter, "JSON resource budget rejected input: {source}")
            }
            JsonDecodeFailure::EmptyInput { .. } => formatter.write_str("JSON input is empty after normalization"),
            JsonDecodeFailure::InvalidUtf8 { source, .. } => match source {
                Some(source) => write!(formatter, "Failed to decode JSON input as UTF-8: {source}"),
                None => formatter.write_str("Failed to decode JSON input as UTF-8"),
            },
            JsonDecodeFailure::InvalidJson { syntax, source, .. } => match source {
                Some(source) => write!(formatter, "Failed to parse JSON: {source}"),
                None => write!(formatter, "Failed to parse JSON: {syntax}"),
            },
            JsonDecodeFailure::UnexpectedTopLevel { expected, actual, .. } => {
                write!(
                    formatter,
                    "Unexpected JSON top-level type: expected {expected}, got {actual}"
                )
            }
            JsonDecodeFailure::Deserialize {
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
            JsonDecodeFailure::Budget { source, .. } => Some(source),
            JsonDecodeFailure::InvalidUtf8 {
                source: Some(source), ..
            } => Some(source),
            JsonDecodeFailure::InvalidJson {
                source: Some(source), ..
            } => Some(source.as_ref()),
            JsonDecodeFailure::Deserialize {
                source: Some(source), ..
            } => Some(source.as_ref()),
            _ => None,
        }
    }
}
