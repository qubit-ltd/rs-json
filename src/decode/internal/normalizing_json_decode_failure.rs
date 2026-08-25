// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines the private failure states of normalizing JSON decoding.

use std::error::Error;
use std::sync::Arc;

use qubit_budget::MeasuredBudgetError;
use qubit_budget::json::JsonResource;

use super::super::JsonRootKind;

/// Stores the mutually exclusive states of a lenient decoding failure.
#[derive(Debug)]
pub(in crate::decode) enum NormalizingJsonDecodeFailure {
    /// Raw input was not valid UTF-8.
    InvalidUtf8 {
        /// Number of bytes in the original input slice.
        raw_input_bytes: usize,
        /// Offset of the first invalid byte reported by UTF-8 validation.
        valid_up_to: usize,
        /// Length of the invalid UTF-8 sequence, when the decoder knows it.
        error_len: Option<usize>,
        /// Original UTF-8 validation error, retained for diagnostic formatting.
        source: Option<std::str::Utf8Error>,
    },
    /// Raw input exceeded its configured byte limit.
    InputTooLarge {
        /// Number of bytes supplied before normalization.
        raw_input_bytes: usize,
        /// Configured maximum raw input size.
        maximum: usize,
    },
    /// Normalized input exceeded its configured byte limit.
    NormalizedInputTooLarge {
        /// Number of bytes in the original input slice.
        raw_input_bytes: usize,
        /// Number of bytes produced by normalization.
        normalized_input_bytes: usize,
        /// Configured maximum normalized input size.
        maximum: usize,
    },
    /// Input was empty at a normalization boundary.
    EmptyInput {
        /// Number of bytes in the original input slice.
        raw_input_bytes: usize,
        /// Normalized length, when normalization completed before emptiness was
        /// detected.
        normalized_input_bytes: Option<usize>,
    },
    /// Normalized text was not valid JSON.
    InvalidJson {
        /// Number of bytes in the original input slice.
        raw_input_bytes: usize,
        /// Number of bytes in the normalized JSON text.
        normalized_input_bytes: usize,
        /// One-based line reported by the JSON parser.
        line: usize,
        /// One-based column reported by the JSON parser.
        column: usize,
        /// Parser error retained for diagnostic formatting.
        source: Option<Arc<dyn Error + Send + Sync>>,
    },
    /// Decoded JSON exceeded a resource budget.
    Budget {
        /// Number of bytes in the original input slice.
        raw_input_bytes: usize,
        /// Normalized input length, when normalization completed.
        normalized_input_bytes: Option<usize>,
        /// Measured resource rejection from the decode transaction.
        source: MeasuredBudgetError<JsonResource, usize>,
    },
    /// Valid JSON had an unexpected top-level kind.
    UnexpectedTopLevel {
        /// Number of bytes in the original input slice.
        raw_input_bytes: usize,
        /// Number of bytes in the normalized JSON text.
        normalized_input_bytes: usize,
        /// Top-level kind required by the requested decode operation.
        expected: JsonRootKind,
        /// Top-level kind found in the normalized document.
        actual: JsonRootKind,
    },
    /// Valid JSON could not deserialize into the requested type.
    Deserialize {
        /// Number of bytes in the original input slice.
        raw_input_bytes: usize,
        /// Number of bytes in the normalized JSON text.
        normalized_input_bytes: usize,
        /// One-based line reported by the deserializer.
        line: usize,
        /// One-based column reported by the deserializer.
        column: usize,
        /// Deserialization error retained for diagnostic formatting.
        source: Option<Arc<dyn Error + Send + Sync>>,
    },
}
