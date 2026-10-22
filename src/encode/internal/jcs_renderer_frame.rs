// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Active container state used by the iterative JCS renderer.

use serde_json::Value;

/// Tracks traversal progress for one non-empty JSON container on the renderer
/// stack.
pub(super) enum JcsRendererFrame<'value> {
    /// Tracks the next element in an array while its children are rendered.
    Array {
        /// Values borrowed from the source array.
        values: &'value [Value],
        /// Index of the next array value to visit.
        next: usize,
    },
    /// Tracks the next member in an object after its keys have been sorted for
    /// JCS.
    Object {
        /// Object members sorted by their UTF-16 encoded keys.
        entries: Vec<(&'value str, &'value Value)>,
        /// Index of the next sorted member to visit.
        next: usize,
    },
}
