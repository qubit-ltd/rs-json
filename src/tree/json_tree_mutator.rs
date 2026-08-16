// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Implements non-recursive, mutable JSON tree processing.

use std::ptr::NonNull;

use qubit_budget::MeasuredBudgetError;
use qubit_budget::ResourceQuantity;
use qubit_budget::json::JsonMeasurement;
use qubit_budget::json::JsonValueTransaction;
use serde_json::Value;
use serde_json::map::IterMut;

use super::JsonTreeBudgetRejection;
use super::JsonTreeContext;
use super::JsonTreeControl;
use super::JsonTreeLocation;
use super::JsonTreeMutVisitor;
use super::JsonTreeProcessError;

/// Mutates JSON values while borrowing one staged JSON value transaction.
pub struct JsonTreeMutator<'transaction, 'budget, R, Q>
where
    Q: ResourceQuantity,
{
    transaction: &'transaction mut JsonValueTransaction<'budget, R, Q>,
}

impl<'transaction, 'budget, R, Q> JsonTreeMutator<'transaction, 'budget, R, Q>
where
    R: Clone,
    Q: ResourceQuantity,
{
    /// Creates a mutator borrowing the supplied JSON value transaction.
    pub fn new(
        transaction: &'transaction mut JsonValueTransaction<'budget, R, Q>,
    ) -> Self {
        Self { transaction }
    }

    /// Mutates every admitted node in depth-first order without Rust recursion.
    pub fn process<V>(
        &mut self,
        root: &mut Value,
        visitor: &mut V,
    ) -> Result<(), JsonTreeProcessError<R, Q, V::Error>>
    where
        V: JsonTreeMutVisitor<R, Q>,
    {
        let mut stack = vec![MutFrame::root(root)];
        while !stack.is_empty() {
            let index = stack.len() - 1;
            if !stack[index].entered {
                let frame = &mut stack[index];
                let context = frame.location.context(frame.depth);
                // SAFETY: frames are made from the caller's exclusive root
                // borrow; no ancestor container is structurally
                // changed while a child is live.
                let value = unsafe { frame.value.as_mut() };
                if let Some(key) = frame.location.key()
                    && let Err(error) = self
                        .transaction
                        .try_admit(JsonMeasurement::Key { bytes: key.len() })
                {
                    let rejection = visitor
                        .reject_budget(value, context, &error)
                        .map_err(JsonTreeProcessError::Visitor)?;
                    if rejection == JsonTreeBudgetRejection::Abort {
                        return Err(JsonTreeProcessError::Budget(error));
                    }
                    frame.entered = true;
                    frame.finished = true;
                    continue;
                }
                if let Err(error) = self.admit(value, frame.depth) {
                    let rejection = visitor
                        .reject_budget(value, context, &error)
                        .map_err(JsonTreeProcessError::Visitor)?;
                    if rejection == JsonTreeBudgetRejection::Abort {
                        return Err(JsonTreeProcessError::Budget(error));
                    }
                    frame.entered = true;
                    frame.finished = true;
                    continue;
                }
                let control = visitor
                    .visit(value, context)
                    .map_err(JsonTreeProcessError::Visitor)?;
                frame.entered = true;
                if control != JsonTreeControl::Descend {
                    frame.finished = true;
                }
                continue;
            }
            if let Some(child) = stack[index].next_child() {
                stack.push(child);
                continue;
            }
            let _ = stack.pop().expect("mutable frame exists");
        }
        Ok(())
    }

    /// Admits one node before invoking its mutable visitor callback.
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

/// Owns the location information needed while mutable traversal advances.
#[derive(Clone)]
enum OwnedLocation {
    Root,
    ArrayElement(usize),
    ObjectValue(String),
}

impl OwnedLocation {
    /// Borrows this owned location for a visitor context.
    fn context(&self, depth: usize) -> JsonTreeContext<'_> {
        let location = match self {
            Self::Root => JsonTreeLocation::Root,
            Self::ArrayElement(index) => {
                JsonTreeLocation::ArrayElement { index: *index }
            }
            Self::ObjectValue(key) => JsonTreeLocation::ObjectValue { key },
        };
        JsonTreeContext { depth, location }
    }
    /// Returns the object key that must be charged before child admission.
    fn key(&self) -> Option<&str> {
        match self {
            Self::ObjectValue(key) => Some(key),
            Self::Root | Self::ArrayElement(_) => None,
        }
    }
}

/// Tracks one in-place mutable traversal position.
struct MutFrame {
    location: OwnedLocation,
    depth: usize,
    value: NonNull<Value>,
    entered: bool,
    finished: bool,
    cursor: Option<MutChildCursor>,
}

impl MutFrame {
    /// Creates the root frame from the exclusive mutable root borrow.
    fn root(value: &mut Value) -> Self {
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
    fn child(location: OwnedLocation, depth: usize, value: &mut Value) -> Self {
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
    fn next_child(&mut self) -> Option<Self> {
        if self.finished {
            return None;
        }
        let cursor = self
            .cursor
            .get_or_insert_with(|| MutChildCursor::new(self.value));
        cursor.next(self.depth)
    }
}

/// Lazily yields mutable children without detaching their parent container.
enum MutChildCursor {
    Array {
        values: NonNull<Vec<Value>>,
        next: usize,
    },
    Object {
        iter: IterMut<'static>,
    },
    Empty,
}

impl MutChildCursor {
    /// Creates a cursor whose references remain inside the borrowed root.
    fn new(mut value: NonNull<Value>) -> Self {
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
                let iter = unsafe {
                    std::mem::transmute::<IterMut<'_>, IterMut<'static>>(iter)
                };
                Self::Object { iter }
            }
            Value::Null
            | Value::Bool(_)
            | Value::Number(_)
            | Value::String(_) => Self::Empty,
        }
    }
    /// Returns the next child and its traversal metadata.
    fn next(&mut self, parent_depth: usize) -> Option<MutFrame> {
        let depth = parent_depth.checked_add(1).expect(
            "a materialized JSON tree cannot have usize::MAX nesting depth",
        );
        match self {
            Self::Array { values, next } => {
                // SAFETY: the parent vector is structurally unchanged while its
                // cursor is live.
                let values = unsafe { values.as_mut() };
                let index = *next;
                let child = values.get_mut(index)?;
                *next = next.checked_add(1)?;
                Some(MutFrame::child(
                    OwnedLocation::ArrayElement(index),
                    depth,
                    child,
                ))
            }
            Self::Object { iter } => iter.next().map(|(key, child)| {
                MutFrame::child(
                    OwnedLocation::ObjectValue(key.clone()),
                    depth,
                    child,
                )
            }),
            Self::Empty => None,
        }
    }
}
