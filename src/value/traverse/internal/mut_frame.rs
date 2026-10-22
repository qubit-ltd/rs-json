// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines one stack frame for mutable JSON traversal.
//!
//! # Safety invariants
//!
//! Every frame pointer identifies a node inside the exclusively borrowed root
//! and remains valid until that frame is popped. Frames may move as the stack
//! reallocates because moving a frame does not move the pointed-to JSON node.
//! While a child frame is active, ancestor frames retain only pointers and
//! suspended cursors; they do not dereference or expose their nodes.

use std::ptr::NonNull;

use serde_json::Value;

use super::MutChildCursor;
use super::OwnedLocation;

/// Tracks one in-place mutable traversal position.
#[must_use]
pub(in crate::value::traverse) struct MutFrame {
    /// Owned location reported to the mutation visitor.
    pub(in crate::value::traverse) location: OwnedLocation,
    /// Root-inclusive depth of the frame's value.
    pub(in crate::value::traverse) depth: usize,
    /// Non-null pointer to the value being processed in place.
    pub(in crate::value::traverse) value: NonNull<Value>,
    /// Whether the visitor has entered this value.
    pub(in crate::value::traverse) entered: bool,
    /// Whether all children and the leave callback have completed.
    pub(in crate::value::traverse) finished: bool,
    /// Lazy cursor over the value's mutable children.
    pub(in crate::value::traverse) cursor: Option<MutChildCursor>,
}

impl MutFrame {
    /// Creates the root frame from the exclusive mutable root borrow.
    ///
    /// # Parameters
    ///
    /// * `value` - Exclusively borrowed root JSON value tracked by this frame.
    ///
    /// # Returns
    ///
    /// An unentered, unfinished root frame at depth one.
    #[inline(always)]
    pub(in crate::value::traverse) fn root(value: &mut Value) -> Self {
        Self {
            location: OwnedLocation::Root,
            depth: 1,
            value: NonNull::from(value),
            entered: false,
            finished: false,
            cursor: None,
        }
    }
    /// Creates one child frame retaining the parent-derived value pointer.
    ///
    /// # Parameters
    ///
    /// * `location` - Root-relative path identifying the child.
    /// * `depth` - Root-inclusive nesting depth of the child value.
    /// * `value` - Exclusively borrowed child value retained by this frame.
    ///
    /// # Returns
    ///
    /// An unentered, unfinished child frame at the supplied location and depth.
    #[inline(always)]
    pub(in crate::value::traverse) fn child(location: OwnedLocation, depth: usize, value: &mut Value) -> Self {
        Self {
            location,
            depth,
            value: NonNull::from(value),
            entered: false,
            finished: false,
            cursor: None,
        }
    }
    /// Returns the next mutable child while preserving the parent container.
    ///
    /// # Returns
    ///
    /// Returns `Some` with the next child frame when an unvisited child exists,
    /// or `None` when this frame is finished or has no remaining children.
    pub(in crate::value::traverse) fn next_child(&mut self) -> Option<Self> {
        if self.finished {
            return None;
        }
        let cursor = self.cursor.get_or_insert_with(|| MutChildCursor::new(self.value));
        cursor.next(self.depth)
    }
}
