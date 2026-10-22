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
pub(super) enum DisplayBudgetKind {
    /// Ordinary JSON string payload.
    String,

    /// JSON object key text.
    Key,

    /// Raw JSON source, bounded by the complete output limit while collected.
    RawOutput,
}

impl DisplayBudgetKind {
    /// Returns the budget measurement for prospective text, if it is a JSON
    /// string or key payload.
    pub(super) fn measurement(self, depth: usize, bytes: usize) -> Option<JsonMeasurement> {
        match self {
            Self::String => Some(JsonMeasurement::String { depth, bytes }),
            Self::Key => Some(JsonMeasurement::Key { bytes }),
            Self::RawOutput => None,
        }
    }
}
