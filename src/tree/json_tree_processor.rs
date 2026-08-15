// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Implements non-recursive, read-only JSON tree processing.
// qubit-style: allow multiple-public-types

use std::ptr::NonNull;

use qubit_budget::MeasuredBudgetError;
use qubit_budget::ResourceQuantity;
use qubit_budget::json::JsonMeasurement;
use qubit_budget::json::JsonValueTransaction;
use serde_json::Value;
use serde_json::map::Iter;
use serde_json::map::IterMut;

use super::JsonBudgetRejection;
use super::JsonTreeContext;
use super::JsonTreeControl;
use super::JsonTreeLocation;
use super::JsonTreeMutVisitor;
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
    /// Creates a processor borrowing the supplied JSON value transaction.
    pub fn new(
        transaction: &'transaction mut JsonValueTransaction<'budget, R, Q>,
    ) -> Self {
        Self { transaction }
    }

    /// Processes every node in depth-first order without Rust recursion.
    ///
    /// The explicit `'value` lifetime intentionally keeps the input tree's
    /// stack-held borrow independent from the processor's budget borrow.
    #[allow(clippy::needless_lifetimes)]
    pub fn process<'value, V>(
        &mut self,
        value: &'value Value,
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

    /// Mutates every admitted node in depth-first order without Rust recursion.
    ///
    /// Mutation and budget accounting are incremental. If this method returns
    /// an error, mutations already made by the visitor and budget already
    /// consumed remain in effect; this operation does not roll either back.
    /// The traversal never detaches a child from `root`, so errors and panics
    /// leave the JSON tree structurally valid without rebuilding it.
    pub(crate) fn process_mut<V>(
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
                // SAFETY: `MutFrame::root` and `MutFrame::child` create these
                // pointers from the exclusively borrowed root. No ancestor
                // container is structurally modified while a child frame is
                // live, so the current value remains valid for this callback.
                let value = unsafe { frame.value.as_mut() };

                if let Some(key) = frame.location.key()
                    && let Err(error) = self
                        .transaction
                        .try_admit(JsonMeasurement::Key { bytes: key.len() })
                {
                    let rejection = visitor
                        .reject_budget(value, context, &error)
                        .map_err(JsonTreeProcessError::Visitor)?;
                    if rejection == JsonBudgetRejection::Abort {
                        return Err(JsonTreeProcessError::Budget(error));
                    }
                    frame.entered = true;
                    continue;
                }

                if let Err(error) = self.admit(value, frame.depth) {
                    let rejection = visitor
                        .reject_budget(value, context, &error)
                        .map_err(JsonTreeProcessError::Visitor)?;
                    if rejection == JsonBudgetRejection::Abort {
                        return Err(JsonTreeProcessError::Budget(error));
                    }
                    frame.entered = true;
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

/// Mutates JSON values while borrowing one staged JSON value transaction.
pub struct JsonTreeMutator<'transaction, 'budget, R, Q>
where
    Q: ResourceQuantity,
{
    reader: JsonTreeReader<'transaction, 'budget, R, Q>,
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
        Self {
            reader: JsonTreeReader::new(transaction),
        }
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
        self.reader.process_mut(root, visitor)
    }
}

/// Represents one stack-held frame in a read-only depth-first traversal.
struct ReadFrame<'value> {
    /// Borrowed node visited by this frame.
    value: &'value Value,

    /// Callback context for this node.
    context: JsonTreeContext<'value>,

    /// Current enter, child, or leave phase.
    state: ReadFrameState<'value>,
}

impl<'value> ReadFrame<'value> {
    /// Creates a frame that will enter `value` before scheduling children.
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
    /// The node has not yet been admitted or entered.
    Enter,

    /// The cursor yields children in source order.
    Children(ChildCursor<'value>),

    /// All children have completed and the visitor should leave the node.
    Leave,
}

/// Lazily yields one container's children, keeping pending memory O(depth).
enum ChildCursor<'value> {
    /// Array iterator with the next child index.
    Array {
        /// Next array index and value.
        iter: std::iter::Enumerate<std::slice::Iter<'value, Value>>,
        /// Root-inclusive depth of each child.
        depth: usize,
    },

    /// Object iterator over borrowed keys and values.
    Object {
        /// Next object entry.
        iter: Iter<'value>,
        /// Root-inclusive depth of each child.
        depth: usize,
    },

    /// Scalar node with no children.
    Empty,
}

impl<'value> ChildCursor<'value> {
    /// Creates a cursor for the immediate children of `value`.
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

/// Owns the location information needed while mutable traversal advances.
#[derive(Clone)]
enum OwnedLocation {
    /// The root value.
    Root,
    /// A zero-based array element.
    ArrayElement(usize),
    /// An object value associated with an owned key.
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
    /// Array cursor indexed through the owned vector pointer.
    Array {
        values: NonNull<Vec<Value>>,
        next: usize,
    },
    /// Object cursor retaining a suspended mutable iterator.
    Object { iter: IterMut<'static> },
    /// Scalar node with no children.
    Empty,
}

impl MutChildCursor {
    /// Creates a cursor whose references remain inside the borrowed root.
    fn new(mut value: NonNull<Value>) -> Self {
        // SAFETY: the pointer originates from the caller's exclusive root
        // borrow. This cursor is used only while the frame owns that borrow.
        match unsafe { value.as_mut() } {
            Value::Array(values) => Self::Array {
                values: NonNull::from(values),
                next: 0,
            },
            Value::Object(entries) => {
                let iter = entries.iter_mut();
                // SAFETY: `MutFrame` keeps the root exclusively borrowed for
                // the traversal. Parent object entries are never inserted,
                // removed, or replaced while this iterator is suspended; each
                // yielded child finishes before the iterator advances again.
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
                // SAFETY: the pointer belongs to the exclusively borrowed
                // root, and the parent vector is structurally unchanged while
                // its cursor or child frames are live.
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
