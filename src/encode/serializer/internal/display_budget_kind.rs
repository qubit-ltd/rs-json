// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Budget categories used by strict display-based JSON encoding.

use qubit_budget::json::JsonMeasurement;

/// Resource checked while a `Display` implementation emits text chunks.
#[derive(Clone, Copy)]
pub(in crate::encode) enum DisplayBudgetKind {
    /// Ordinary JSON string payload.
    String,

    /// JSON object key text.
    Key,

    /// Number text checked incrementally and admitted by number preflight.
    Number,

    /// Raw JSON source, bounded by the complete output limit while collected.
    RawOutput,
}

impl DisplayBudgetKind {
    /// Returns the budget measurement for prospective text, if it is a JSON
    /// string, key, or number text.
    ///
    /// # Parameters
    ///
    /// * `self` - Budget category selecting how text is measured.
    /// * `depth` - Nesting depth used when measuring a JSON string payload.
    /// * `bytes` - Number of emitted bytes in the prospective text chunk.
    ///
    /// # Returns
    ///
    /// Returns `Some` with a string, key, or number measurement for `String`,
    /// `Key`, and `Number`.
    /// Returns `None` for `RawOutput`, which is bounded by the complete output
    /// limit instead of a JSON payload measurement.
    #[inline]
    pub(super) fn measurement(self, depth: usize, bytes: usize) -> Option<JsonMeasurement> {
        match self {
            Self::String => Some(JsonMeasurement::String { depth, bytes }),
            Self::Key => Some(JsonMeasurement::Key { bytes }),
            Self::Number => Some(JsonMeasurement::Number { depth, bytes }),
            Self::RawOutput => None,
        }
    }
}
