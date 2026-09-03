// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines the owned sources retained by JSON decoding errors.

use std::error::Error;
use std::fmt;
use std::sync::Arc;

use qubit_budget::MeasuredBudgetError;

use super::JsonDecodeStage;
use super::JsonRootKind;
use super::JsonSyntaxError;

/// An owned semantic source extracted from a JSON decoding failure.
///
/// This enum preserves the complete structured state of exactly one failure.
/// It lets downstream adapters move error data into their own models with one
/// exhaustive `match`, without pairing [`super::JsonDecodeError::kind`] with
/// fallible source accessors. Parser and Serde sources remain present only
/// when decoding used [`super::DiagnosticPolicy::Detailed`].
///
/// # Type Parameters
///
/// * `R` - Resource identity attached to budget failures.
/// * `Q` - Quantity representation attached to budget failures.
///
/// # Examples
///
/// ```
/// use qubit_json::decode::{JsonDecodeErrorSource, JsonDecoder};
///
/// let mut decoder = JsonDecoder::unlimited();
/// let error = decoder.validate_str("{").expect_err("invalid JSON must fail");
/// match error.into_source() {
///     JsonDecodeErrorSource::InvalidJson { syntax, .. } => {
///         assert_eq!(syntax.offset(), 1);
///     }
///     source => panic!("unexpected decoding source: {source:?}"),
/// }
/// ```
#[must_use]
#[derive(Debug, Clone)]
pub enum JsonDecodeErrorSource<R, Q = usize>
where
    Q: Copy + fmt::Debug,
{
    /// A resource measurement was rejected.
    Budget {
        /// Semantic stage where the measurement was rejected.
        stage: JsonDecodeStage,
        /// Original input length.
        raw_input_bytes: usize,
        /// Normalized length when normalization completed.
        normalized_input_bytes: Option<usize>,
        /// Complete measured-budget failure.
        source: MeasuredBudgetError<R, Q>,
    },
    /// Input was empty at a public boundary.
    EmptyInput {
        /// Semantic stage where emptiness was detected.
        stage: JsonDecodeStage,
        /// Original input length.
        raw_input_bytes: usize,
        /// Normalized length when available.
        normalized_input_bytes: Option<usize>,
    },
    /// Raw bytes were not valid UTF-8.
    InvalidUtf8 {
        /// Original input length.
        raw_input_bytes: usize,
        /// Valid prefix length reported by UTF-8 validation.
        valid_up_to: usize,
        /// Invalid sequence length when known.
        error_len: Option<usize>,
        /// Detailed source retained only under detailed diagnostics.
        source: Option<std::str::Utf8Error>,
    },
    /// Text was not one valid JSON document.
    InvalidJson {
        /// Original input length.
        raw_input_bytes: usize,
        /// Normalized length when normalization completed.
        normalized_input_bytes: Option<usize>,
        /// Stable scanner reason and source coordinates.
        syntax: JsonSyntaxError,
        /// Detailed parser source retained only under detailed diagnostics.
        source: Option<Arc<dyn Error + Send + Sync>>,
    },
    /// A valid document had an unexpected top-level kind.
    UnexpectedTopLevel {
        /// Original input length.
        raw_input_bytes: usize,
        /// Normalized length when normalization completed.
        normalized_input_bytes: Option<usize>,
        /// Required top-level kind.
        expected: JsonRootKind,
        /// Observed top-level kind.
        actual: JsonRootKind,
    },
    /// A valid admitted document could not materialize the target type.
    Deserialize {
        /// Original input length.
        raw_input_bytes: usize,
        /// Normalized length when normalization completed.
        normalized_input_bytes: Option<usize>,
        /// One-based Serde line, or zero when unavailable.
        line: usize,
        /// One-based Serde column, or zero when unavailable.
        column: usize,
        /// Detailed source retained only under detailed diagnostics.
        source: Option<Arc<dyn Error + Send + Sync>>,
    },
}
