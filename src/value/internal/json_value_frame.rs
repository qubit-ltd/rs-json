// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
// =============================================================================
//! One guarded open array or object in a lexical JSON value builder.

use crate::value::SafeJsonValue;

/// The container currently being materialized.
pub(crate) enum JsonValueFrame {
    /// An array whose children are independently protected values.
    Array(Vec<SafeJsonValue>),
    /// An object with a decoded key awaiting its corresponding value.
    Object {
        /// Values accumulated for this object.
        values: Vec<(String, SafeJsonValue)>,
        /// Key emitted immediately before the next value.
        pending_key: Option<String>,
    },
}
