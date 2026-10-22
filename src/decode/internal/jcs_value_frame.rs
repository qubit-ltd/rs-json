// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

//! One open array or object in a JCS value builder.

use serde_json::Map;
use serde_json::Value;

/// The container currently being materialized.
pub(in crate::decode) enum JcsValueFrame {
    /// An array whose children are appended in source order.
    Array(Vec<Value>),
    /// An object with a decoded key awaiting its corresponding value.
    Object {
        /// Values accumulated for this object.
        values: Map<String, Value>,
        /// Key emitted immediately before the next value.
        pending_key: Option<String>,
    },
}
