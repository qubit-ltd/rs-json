// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Iterator continuations used by non-recursive JSON value Debug formatting.

use serde_json::Value;

/// One pending formatting operation for a JSON value.
///
/// Container continuations retain only their remaining iterator and whether a
/// child has already been emitted, keeping auxiliary storage proportional to
/// nesting depth. Input and formatter storage are not included in that bound.
pub(in crate::value) enum JsonValueDebugAction<'a> {
    /// Formats one JSON value.
    Value(&'a Value),
    /// Writes a static punctuation fragment.
    Text(&'static str),
    /// Formats an object key using Rust string Debug syntax.
    Key(&'a str),
    /// Continues an array after processing at most one element.
    Array {
        /// Remaining array elements.
        values: std::slice::Iter<'a, Value>,
        /// Whether an earlier element has already been formatted.
        has_emitted_item: bool,
    },
    /// Continues an object after processing at most one entry.
    Object {
        /// Remaining entries in the map's iteration order.
        values: serde_json::map::Iter<'a>,
        /// Whether an earlier entry has already been formatted.
        has_emitted_item: bool,
    },
}
