// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines the lazy child cursor for read-only JSON traversal.

use serde_json::Value;
use serde_json::map::Iter;

use super::super::JsonTreeLocation;

/// Lazily yields one container's children, keeping pending memory O(depth).
pub(in crate::value::traverse) enum ChildCursor<'value> {
    Array {
        iter: std::iter::Enumerate<std::slice::Iter<'value, Value>>,
        depth: usize,
    },
    Object {
        iter: Iter<'value>,
        depth: usize,
    },
    Empty,
}

impl<'value> ChildCursor<'value> {
    /// Creates a cursor for the immediate children of `value`.
    #[inline]
    pub(in crate::value::traverse) fn new(value: &'value Value, depth: usize) -> Self {
        let child_depth = depth
            .checked_add(1)
            .expect("a materialized JSON tree cannot have usize::MAX nesting depth");
        match value {
            Value::Array(values) => Self::Array {
                iter: values.iter().enumerate(),
                depth: child_depth,
            },
            Value::Object(entries) => Self::Object {
                iter: entries.iter(),
                depth: child_depth,
            },
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => Self::Empty,
        }
    }

    /// Returns the next child, its location, and its root-inclusive depth.
    pub(in crate::value::traverse) fn next(&mut self) -> Option<(&'value Value, JsonTreeLocation<'value>, usize)> {
        match self {
            Self::Array { iter, depth } => iter
                .next()
                .map(|(index, value)| (value, JsonTreeLocation::ArrayElement { index }, *depth)),
            Self::Object { iter, depth } => iter
                .next()
                .map(|(key, value)| (value, JsonTreeLocation::ObjectValue { key }, *depth)),
            Self::Empty => None,
        }
    }
}
