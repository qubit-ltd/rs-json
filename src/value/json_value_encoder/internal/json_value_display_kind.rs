// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Measurement kind used while collecting a value encoder Display projection.

/// Selects the JSON measurement used for text emitted by a `Display` value.
///
/// The collector uses this distinction to apply string depth limits to values
/// while measuring map keys without treating them as nested JSON strings.
pub(in crate::value::json_value_encoder) enum JsonValueDisplayKind {
    /// Measures emitted text as a JSON string value at the given nesting depth.
    String {
        /// Serialization depth used when checking the string value budget.
        depth: usize,
    },
    /// Measures emitted text as a JSON map key, without string nesting depth.
    Key,
}
