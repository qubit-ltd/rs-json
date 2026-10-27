// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Container traversal state for validated lexical consumption.

/// Marker for an unclosed parent container in the current lexical traversal.
///
/// An array index advances only when its complete child value ends; an object
/// marker remains until the corresponding child value ends as well.
pub(super) enum ValidatedContainer {
    /// Array child currently expected; incremented after a complete child.
    Array { next_index: usize },
    /// Object whose current key/value child has not yet completed.
    Object,
}
