// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Iterator continuations used by non-recursive JSON value comparison.

use serde_json::Map;
use serde_json::Value;

/// Holds the remaining children of one pair of containers being compared.
///
/// Each frame retains iterators instead of collecting sibling nodes, so the
/// comparison stack grows with nesting depth rather than container width.
pub(in crate::value) enum JsonValueCompareFrame<'a> {
    /// Pairs the remaining elements of arrays whose lengths were checked.
    Array {
        /// Remaining elements from the left array.
        left: std::slice::Iter<'a, Value>,
        /// Remaining elements from the right array.
        right: std::slice::Iter<'a, Value>,
    },
    /// Looks up each left object value by key in the right object.
    Object {
        /// Remaining key/value pairs from the left object.
        left: serde_json::map::Iter<'a>,
        /// Right object used for order-independent key lookup.
        right: &'a Map<String, Value>,
    },
}

impl<'a> JsonValueCompareFrame<'a> {
    /// Returns the next child pair, distinguishes a missing object key, and
    /// reports exhaustion separately.
    ///
    /// The outer `None` means the iterator is exhausted; `Some(None)` means an
    /// object key is absent from the right map; and `Some(Some(pair))` yields
    /// the next child pair.
    pub(in crate::value) fn next_pair(&mut self) -> Option<Option<(&'a Value, &'a Value)>> {
        match self {
            Self::Array { left, right } => match (left.next(), right.next()) {
                (Some(left), Some(right)) => Some(Some((left, right))),
                (None, None) => None,
                _ => unreachable!("array lengths are checked before creating a frame"),
            },
            Self::Object { left, right } => left
                .next()
                .map(|(key, value)| right.get(key).map(|other| (value, other))),
        }
    }
}
