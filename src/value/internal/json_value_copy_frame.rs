// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! A guarded array or object frame used to copy one JSON tree depth-first.

use std::slice::Iter as SliceIter;

use serde_json::Map;
use serde_json::Value;
use serde_json::map::Iter as MapIter;

use super::SafeValueBuilder;
use crate::value::SafeJsonValue;

/// Owns one partially constructed container while its source children are
/// visited.
pub(crate) enum JsonValueCopyFrame<'a> {
    /// An array frame keeps its source iterator and whether it owns a
    /// builder-stack entry.
    Array {
        /// Remaining source array values.
        values: SliceIter<'a, Value>,
        /// Whether this frame has a builder for its remaining siblings.
        has_builder: bool,
    },
    /// An object frame retains one key and whether it owns a builder-stack
    /// entry.
    Object {
        /// Remaining source object entries.
        values: MapIter<'a>,
        /// Key associated with the currently visited child value.
        pending_key: Option<&'a String>,
        /// Whether this frame has a builder for its remaining siblings.
        has_builder: bool,
    },
}

impl<'a> JsonValueCopyFrame<'a> {
    /// Creates an empty output frame over an array's borrowed entries.
    pub(crate) fn array(values: &'a [Value]) -> Self {
        Self::Array {
            values: values.iter(),
            has_builder: false,
        }
    }

    /// Creates an empty output frame over an object's borrowed entries.
    pub(crate) fn object(values: &'a Map<String, Value>) -> Self {
        Self::Object {
            values: values.iter(),
            pending_key: None,
            has_builder: false,
        }
    }

    /// Returns the next child value, retaining its object key until insertion.
    pub(crate) fn next_value(&mut self) -> Option<&'a Value> {
        match self {
            Self::Array { values, .. } => values.next(),
            Self::Object {
                values, pending_key, ..
            } => values.next().map(|(key, value)| {
                *pending_key = Some(key);
                value
            }),
        }
    }

    /// Reports whether source siblings remain after the child already fetched.
    pub(crate) fn has_remaining(&self) -> bool {
        match self {
            Self::Array { values, .. } => values.len() > 0,
            Self::Object { values, .. } => values.len() > 0,
        }
    }

    /// Creates a guarded builder when this container has additional siblings.
    pub(crate) fn start_builder(&mut self, builders: &mut Vec<SafeValueBuilder>) {
        match self {
            Self::Array { has_builder, .. } => {
                builders.push(SafeValueBuilder::array());
                *has_builder = true;
            }
            Self::Object { has_builder, .. } => {
                builders.push(SafeValueBuilder::object());
                *has_builder = true;
            }
        }
    }

    /// Adds a completed child, returning a finished owner for a single-child
    /// container.
    pub(crate) fn push_child(
        &mut self,
        child: SafeJsonValue,
        builders: &mut [SafeValueBuilder],
    ) -> Option<SafeJsonValue> {
        match self {
            Self::Array { has_builder: true, .. } => {
                builders
                    .last_mut()
                    .expect("an active array frame has its builder")
                    .push(child);
                None
            }
            Self::Array { has_builder: false, .. } => {
                let mut builder = SafeValueBuilder::array();
                builder.push(child);
                Some(SafeJsonValue::new(builder.finish()))
            }
            Self::Object {
                has_builder: true,
                pending_key,
                ..
            } => {
                builders
                    .last_mut()
                    .expect("an active object frame has its builder")
                    .insert(
                        pending_key
                            .take()
                            .expect("an object child always has a pending key")
                            .clone(),
                        child,
                    );
                None
            }
            Self::Object {
                has_builder: false,
                pending_key,
                ..
            } => {
                let mut builder = SafeValueBuilder::object();
                builder.insert(
                    pending_key
                        .take()
                        .expect("an object child always has a pending key")
                        .clone(),
                    child,
                );
                Some(SafeJsonValue::new(builder.finish()))
            }
        }
    }

    /// Finishes an empty container without creating an active builder frame.
    pub(crate) fn finish_empty(self) -> SafeJsonValue {
        match self {
            Self::Array { .. } => SafeJsonValue::new(SafeValueBuilder::array().finish()),
            Self::Object { .. } => SafeJsonValue::new(SafeValueBuilder::object().finish()),
        }
    }

    /// Finishes a container whose builder is the last active builder.
    pub(crate) fn finish(self, builders: &mut Vec<SafeValueBuilder>) -> SafeJsonValue {
        match self {
            Self::Array { has_builder: true, .. } | Self::Object { has_builder: true, .. } => {}
            Self::Array { has_builder: false, .. } | Self::Object { has_builder: false, .. } => {
                unreachable!("single-child frames finish when their child arrives")
            }
        }
        let builder = builders.pop().expect("an active frame always has its builder");
        SafeJsonValue::new(builder.finish())
    }
}
