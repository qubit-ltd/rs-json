// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines one stack frame for mutable JSON traversal.

use std::ptr::NonNull;

use serde_json::Value;

use super::MutChildCursor;
use super::OwnedLocation;

/// Tracks one in-place mutable traversal position.
pub(in crate::value::traverse) struct MutFrame {
    pub(in crate::value::traverse) location: OwnedLocation,
    pub(in crate::value::traverse) depth: usize,
    pub(in crate::value::traverse) value: NonNull<Value>,
    pub(in crate::value::traverse) entered: bool,
    pub(in crate::value::traverse) finished: bool,
    pub(in crate::value::traverse) cursor: Option<MutChildCursor>,
}

impl MutFrame {
    /// Creates the root frame from the exclusive mutable root borrow.
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
    pub(in crate::value::traverse) fn next_child(&mut self) -> Option<Self> {
        if self.finished {
            return None;
        }
        let cursor = self.cursor.get_or_insert_with(|| MutChildCursor::new(self.value));
        cursor.next(self.depth)
    }
}
