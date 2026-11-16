// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines one stack frame for read-only JSON traversal.

use serde_json::Value;

use super::super::JsonTreeContext;
use super::ReadFrameState;

/// Represents one stack-held frame in a read-only depth-first traversal.
///
/// # Type Parameters
///
/// * `'value` - Borrow lifetime shared by the value, callback context, and
///   traversal state.
pub(in crate::value::traverse) struct ReadFrame<'value> {
    /// Borrowed value associated with this frame.
    pub(in crate::value::traverse) value: &'value Value,
    /// Location and depth passed to visitor callbacks.
    pub(in crate::value::traverse) context: JsonTreeContext<'value>,
    /// Current enter/children/leave phase for this frame.
    pub(in crate::value::traverse) state: ReadFrameState<'value>,
}

impl<'value> ReadFrame<'value> {
    /// Creates a frame that will enter `value` before scheduling children.
    ///
    /// # Parameters
    ///
    /// * `value` - JSON value visited by this frame.
    /// * `context` - Location and depth supplied to callbacks for this value.
    ///
    /// # Returns
    ///
    /// A frame in the enter phase for the supplied value.
    #[must_use]
    #[inline(always)]
    pub(in crate::value::traverse) fn enter(value: &'value Value, context: JsonTreeContext<'value>) -> Self {
        Self {
            value,
            context,
            state: ReadFrameState::Enter,
        }
    }
}
