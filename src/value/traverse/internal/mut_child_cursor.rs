// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines the lazy child cursor for mutable JSON traversal.

use std::ptr::NonNull;

use serde_json::Value;
use serde_json::map::IterMut;

use super::MutFrame;
use super::OwnedLocation;

/// Lazily yields mutable children without detaching their parent container.
pub(in crate::value::traverse) enum MutChildCursor {
    Array { values: NonNull<Vec<Value>>, next: usize },
    Object { iter: IterMut<'static> },
    Empty,
}

impl MutChildCursor {
    /// Creates a cursor whose references remain inside the borrowed root.
    pub(in crate::value::traverse) fn new(mut value: NonNull<Value>) -> Self {
        // SAFETY: the pointer originates from the caller's exclusive root
        // borrow.
        match unsafe { value.as_mut() } {
            Value::Array(values) => Self::Array {
                values: NonNull::from(values),
                next: 0,
            },
            Value::Object(entries) => {
                let iter = entries.iter_mut();
                // SAFETY: no entry is inserted, removed, or replaced while this
                // iterator is suspended.
                let iter = unsafe { std::mem::transmute::<IterMut<'_>, IterMut<'static>>(iter) };
                Self::Object { iter }
            }
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => Self::Empty,
        }
    }
    /// Returns the next child and its traversal metadata.
    pub(in crate::value::traverse) fn next(&mut self, parent_depth: usize) -> Option<MutFrame> {
        let depth = parent_depth
            .checked_add(1)
            .expect("a materialized JSON tree cannot have usize::MAX nesting depth");
        match self {
            Self::Array { values, next } => {
                // SAFETY: the parent vector is structurally unchanged while its
                // cursor is live.
                let values = unsafe { values.as_mut() };
                let index = *next;
                let child = values.get_mut(index)?;
                *next = next.checked_add(1)?;
                Some(MutFrame::child(OwnedLocation::ArrayElement(index), depth, child))
            }
            Self::Object { iter } => iter
                .next()
                .map(|(key, child)| MutFrame::child(OwnedLocation::ObjectValue(key.clone()), depth, child)),
            Self::Empty => None,
        }
    }
}
