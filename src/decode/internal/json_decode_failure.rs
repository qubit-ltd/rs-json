// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines mutually exclusive private JSON decoding failure states.

use std::error::Error;
use std::sync::Arc;

use qubit_budget::MeasuredBudgetError;

use super::super::JsonDecodeStage;
use super::super::JsonRootKind;
use super::super::JsonSyntaxError;

/// Stores diagnostics valid for exactly one JSON decoding failure.
#[derive(Debug, Clone)]
pub(in crate::decode) enum JsonDecodeFailure<R, Q>
where
    Q: Copy + std::fmt::Debug,
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
