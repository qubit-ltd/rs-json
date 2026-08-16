// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Implements non-recursive, read-only JSON tree processing.

use qubit_budget::MeasuredBudgetError;
use qubit_budget::ResourceQuantity;
use qubit_budget::json::JsonMeasurement;
use qubit_budget::json::JsonValueTransaction;
use serde_json::Value;
use serde_json::map::Iter;

use super::JsonTreeContext;
use super::JsonTreeLocation;
use super::JsonTreeProcessError;
use super::JsonTreeVisitor;

/// Processes JSON values while borrowing one staged JSON value transaction.
pub struct JsonTreeReader<'transaction, 'budget, R, Q>
where
    Q: ResourceQuantity,
{
    transaction: &'transaction mut JsonValueTransaction<'budget, R, Q>,
}

impl<'transaction, 'budget, R, Q> JsonTreeReader<'transaction, 'budget, R, Q>
where
    R: Clone,
    Q: ResourceQuantity,
{
    /// Creates a reader borrowing the supplied JSON value transaction.
    ///
    /// # Parameters
    ///
    /// * `transaction` - Transaction receiving node and payload charges.
    ///
    /// # Returns
    ///
    /// A reader borrowing `transaction` for its lifetime.
    #[inline(always)]
    #[must_use]
    pub fn new(
        transaction: &'transaction mut JsonValueTransaction<'budget, R, Q>,
    ) -> Self {
        Self { transaction }
    }

    /// Processes every node in depth-first order without Rust recursion.
    ///
    /// # Type Parameters
    ///
    /// * `V` - Visitor receiving admitted-node callbacks.
    ///
    /// # Parameters
    ///
    /// * `value` - Root JSON value to process.
    /// * `visitor` - Visitor invoked around each admitted node.
    ///
    /// # Returns
    ///
    /// `Ok(())` after the complete tree is processed.
    ///
    /// # Errors
    ///
    /// Returns [`JsonTreeProcessError::Budget`] when resource admission fails,
    /// or [`JsonTreeProcessError::Visitor`] when the visitor rejects a node.
    #[must_use]
    pub fn process<V>(
        &mut self,
        value: &Value,
        visitor: &mut V,
    ) -> Result<(), JsonTreeProcessError<R, Q, V::Error>>
    where
        V: JsonTreeVisitor,
    {
        let mut pending = vec![ReadFrame::enter(
            value,
            JsonTreeContext {
                depth: 1,
                location: JsonTreeLocation::Root,
            },
        )];
        while let Some(frame) = pending.last_mut() {
            match &mut frame.state {
                ReadFrameState::Enter => {
                    let value = frame.value;
                    let context = frame.context;
                    if let JsonTreeLocation::ObjectValue { key } =
                        context.location
                    {
                        self.transaction.try_admit(JsonMeasurement::Key {
                            bytes: key.len(),
                        })?;
                    }
                    self.admit(value, context.depth)?;
                    visitor
                        .enter(value, context)
                        .map_err(JsonTreeProcessError::Visitor)?;
                    frame.state = ReadFrameState::Children(ChildCursor::new(
                        value,
                        context.depth,
                    ));
                }
                ReadFrameState::Children(cursor) => {
                    if let Some((value, location, depth)) = cursor.next() {
                        pending.push(ReadFrame::enter(
                            value,
                            JsonTreeContext { depth, location },
                        ));
                    } else {
                        frame.state = ReadFrameState::Leave;
                    }
                }
                ReadFrameState::Leave => {
                    let frame = pending.pop().expect("read frame exists");
                    visitor
                        .leave(frame.value, frame.context)
                        .map_err(JsonTreeProcessError::Visitor)?;
                }
            }
        }
        Ok(())
    }

    /// Admits one node before any visitor callback.
    fn admit(
        &mut self,
        value: &Value,
        depth: usize,
    ) -> Result<(), MeasuredBudgetError<R, Q>> {
        let measurement = match value {
            Value::Null => JsonMeasurement::Null { depth },
            Value::Bool(_) => JsonMeasurement::Boolean { depth },
            Value::Number(number) => JsonMeasurement::Number {
                depth,
                bytes: number.as_str().len(),
            },
            Value::String(text) => JsonMeasurement::String {
                depth,
                bytes: text.len(),
            },
            Value::Array(values) => JsonMeasurement::Array {
                depth,
                items: values.len(),
            },
            Value::Object(entries) => JsonMeasurement::Object {
                depth,
                entries: entries.len(),
            },
        };
        self.transaction.try_admit(measurement)
    }
}

/// Represents one stack-held frame in a read-only depth-first traversal.
struct ReadFrame<'value> {
    value: &'value Value,
    context: JsonTreeContext<'value>,
    state: ReadFrameState<'value>,
}

impl<'value> ReadFrame<'value> {
    /// Creates a frame that will enter `value` before scheduling children.
    #[inline(always)]
    fn enter(value: &'value Value, context: JsonTreeContext<'value>) -> Self {
        Self {
            value,
            context,
            state: ReadFrameState::Enter,
        }
    }
}

/// Current phase of a read-only traversal frame.
enum ReadFrameState<'value> {
    Enter,
    Children(ChildCursor<'value>),
    Leave,
}

/// Lazily yields one container's children, keeping pending memory O(depth).
enum ChildCursor<'value> {
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
    fn new(value: &'value Value, depth: usize) -> Self {
        let child_depth = depth.checked_add(1).expect(
            "a materialized JSON tree cannot have usize::MAX nesting depth",
        );
        match value {
            Value::Array(values) => Self::Array {
                iter: values.iter().enumerate(),
                depth: child_depth,
            },
            Value::Object(entries) => Self::Object {
                iter: entries.iter(),
                depth: child_depth,
            },
            Value::Null
            | Value::Bool(_)
            | Value::Number(_)
            | Value::String(_) => Self::Empty,
        }
    }

    /// Returns the next child, its location, and its root-inclusive depth.
    fn next(
        &mut self,
    ) -> Option<(&'value Value, JsonTreeLocation<'value>, usize)> {
        match self {
            Self::Array { iter, depth } => iter.next().map(|(index, value)| {
                (value, JsonTreeLocation::ArrayElement { index }, *depth)
            }),
            Self::Object { iter, depth } => iter.next().map(|(key, value)| {
                (value, JsonTreeLocation::ObjectValue { key }, *depth)
            }),
            Self::Empty => None,
        }
    }
}
