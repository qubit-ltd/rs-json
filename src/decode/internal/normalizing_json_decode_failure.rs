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
        raw_input_bytes: usize,
        valid_up_to: usize,
        error_len: Option<usize>,
        source: Option<std::str::Utf8Error>,
    },
    /// Raw input exceeded its configured byte limit.
    InputTooLarge { raw_input_bytes: usize, maximum: usize },
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
        expected: JsonRootKind,
        actual: JsonRootKind,
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
