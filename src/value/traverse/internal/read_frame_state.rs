// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines the phase of a read-only traversal frame.

use super::ChildCursor;

/// Current phase of a read-only traversal frame.
pub(in crate::value::traverse) enum ReadFrameState<'value> {
    /// The frame is ready for its enter callback.
    Enter,
    /// The frame is enumerating its immediate children.
    Children(
        /// Cursor over the frame's immediate children.
        ChildCursor<'value>,
    ),
    /// All children were visited and the leave callback is ready.
    Leave,
}
