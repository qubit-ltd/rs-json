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

use super::super::JsonTreeControl;
use super::super::JsonTreeMutVisitor;
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
    value: NonNull<Value>,
    /// Whether the visitor has entered this value.
    pub(in crate::value::traverse) entered: bool,
    /// Whether all children and the leave callback have completed.
    pub(in crate::value::traverse) finished: bool,
    /// Lazy cursor over the value's mutable children.
    cursor: Option<MutChildCursor>,
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
    ///
    /// # Safety
    ///
    /// The root value must remain alive and at the same address until this
    /// frame and all cursors and child frames derived from it are dropped.
    /// The caller must keep the root exclusively borrowed for that duration.
    #[inline(always)]
    pub(in crate::value::traverse) unsafe fn root(value: &mut Value) -> Self {
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
    ///
    /// # Safety
    ///
    /// The child value must remain alive and at the same address until this
    /// frame and all cursors and descendant frames derived from it are dropped.
    /// Any ancestor frame must stay suspended while this frame is active.
    #[inline(always)]
    pub(in crate::value::traverse) unsafe fn child(location: OwnedLocation, depth: usize, value: &mut Value) -> Self {
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
    ///
    /// # Safety
    ///
    /// The caller must resume this frame only when it is active and its visitor
    /// has returned. Any previously yielded child frame and all of that
    /// child's descendants must have been dropped. While this frame's cursor
    /// exists, its node's container structure and backing allocation must
    /// remain stable, and ancestor frames must remain suspended.
    pub(in crate::value::traverse) unsafe fn next_child(&mut self) -> Option<Self> {
        if self.finished {
            return None;
        }
        // SAFETY: this frame is active after its visitor has returned, and its
        // node remains stable until this cursor and all yielded children drop.
        let cursor = self
            .cursor
            .get_or_insert_with(|| unsafe { MutChildCursor::new(self.value) });
        // SAFETY: the caller resumes this frame only after its previously
        // yielded child and all descendants have been removed from the stack.
        unsafe { cursor.next(self.depth) }
    }

    /// Visits this frame's node without exposing its pointer to the caller.
    ///
    /// # Safety
    ///
    /// Call only for an unentered frame with no cursor and no active
    /// descendants. Ancestor frames must remain suspended, and this node must
    /// remain inside the exclusively borrowed root for the duration of the
    /// callback. The callback borrow must end before a child cursor is created.
    pub(in crate::value::traverse) unsafe fn visit<V>(&mut self, visitor: &mut V) -> Result<JsonTreeControl, V::Error>
    where
        V: JsonTreeMutVisitor,
    {
        let context = self.location.context(self.depth);
        // SAFETY: the active frame has no cursor or live descendant, its node
        // remains inside the exclusively borrowed root, and the callback borrow
        // ends before a child cursor is created.
        let value = unsafe { self.value.as_mut() };
        visitor.visit(value, context)
    }
}
