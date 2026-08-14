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
use qubit_budget::json::JsonValueBudget;
use serde_json::Value;
use serde_json::map::Iter;

use super::JsonBudgetRejection;
use super::JsonTreeContext;
use super::JsonTreeControl;
use super::JsonTreeLocation;
use super::JsonTreeMutVisitor;
use super::JsonTreeProcessError;
use super::JsonTreeVisitor;

/// Processes JSON values while borrowing one shared JSON value budget.
pub struct JsonTreeProcessor<'a, R, Q>
where
    Q: ResourceQuantity,
{
    budget: &'a mut JsonValueBudget<R, Q>,
}

impl<'a, R, Q> JsonTreeProcessor<'a, R, Q>
where
    R: Clone,
    Q: ResourceQuantity,
{
    /// Creates a processor borrowing the supplied JSON value budget.
    pub fn new(budget: &'a mut JsonValueBudget<R, Q>) -> Self {
        Self { budget }
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
                        self.budget.consume_key_bytes_usize(key.len())?;
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
    /// The internal restoration guard only rebuilds detached tree structure so
    /// `root` remains a valid [`Value`], and does not restore its original
    /// contents.
    pub fn process_mut<V>(
        &mut self,
        root: &mut Value,
        visitor: &mut V,
    ) -> Result<(), JsonTreeProcessError<R, Q, V::Error>>
    where
        V: JsonTreeMutVisitor<R, Q>,
    {
        let value = std::mem::take(root);
        let mut guard = RootRestoreGuard::new(root);
        guard.stack.push(MutFrame::root(value));

        while !guard.stack.is_empty() {
            let index = guard.stack.len() - 1;
            if !guard.stack[index].entered {
                let frame = &mut guard.stack[index];
                let context = frame.location.context(frame.depth);

                if let Some(key) = frame.location.key()
                    && let Err(error) =
                        self.budget.consume_key_bytes_usize(key.len())
                {
                    let rejection = visitor
                        .reject_budget(&mut frame.value, context, &error)
                        .map_err(JsonTreeProcessError::Visitor)?;
                    if rejection == JsonBudgetRejection::Abort {
                        return Err(JsonTreeProcessError::Budget(error));
                    }
                    frame.entered = true;
                    continue;
                }

                if let Err(error) = self.admit(&frame.value, frame.depth) {
                    let rejection = visitor
                        .reject_budget(&mut frame.value, context, &error)
                        .map_err(JsonTreeProcessError::Visitor)?;
                    if rejection == JsonBudgetRejection::Abort {
                        return Err(JsonTreeProcessError::Budget(error));
                    }
                    frame.entered = true;
                    continue;
                }

                let control = visitor
                    .visit(&mut frame.value, context)
                    .map_err(JsonTreeProcessError::Visitor)?;
                frame.entered = true;
                if control == JsonTreeControl::Descend {
                    frame.children =
                        take_children(&mut frame.value, frame.depth);
                }
                continue;
            }

            if let Some(child) = guard.stack[index].children.pop() {
                guard.stack.push(MutFrame::child(child));
                continue;
            }

            let frame = guard.stack.pop().expect("mutable frame exists");
            if let Some(parent) = guard.stack.last_mut() {
                insert_child(&mut parent.value, frame.location, frame.value);
            } else {
                guard.finish(frame.value);
            }
        }
        Ok(())
    }

    /// Returns the borrowed budget.
    pub const fn budget(&self) -> &JsonValueBudget<R, Q> {
        self.budget
    }

    /// Returns the borrowed budget for direct caller-managed accounting.
    pub fn budget_mut(&mut self) -> &mut JsonValueBudget<R, Q> {
        self.budget
    }

    /// Admits one node before any visitor callback.
    fn admit(
        &mut self,
        value: &Value,
        depth: usize,
    ) -> Result<(), MeasuredBudgetError<R, Q>> {
        match value {
            Value::Null | Value::Bool(_) => self.budget.enter_node_usize(depth),
            Value::Number(number) => {
                self.budget.enter_number_usize(depth, number.as_str().len())
            }
            Value::String(text) => {
                self.budget.enter_string_usize(depth, text.len())
            }
            Value::Array(values) => {
                self.budget.enter_array_usize(depth, values.len())
            }
            Value::Object(entries) => {
                self.budget.enter_object_usize(depth, entries.len())
            }
        }
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

/// Owns one detached value and the child values still awaiting processing.
struct MutFrame {
    location: OwnedLocation,
    depth: usize,
    value: Value,
    children: Vec<ChildEntry>,
    entered: bool,
}

impl MutFrame {
    fn root(value: Value) -> Self {
        Self {
            location: OwnedLocation::Root,
            depth: 1,
            value,
            children: Vec::new(),
            entered: false,
        }
    }

    fn child(child: ChildEntry) -> Self {
        let depth = child.depth;
        Self {
            location: child.location,
            depth,
            value: child.value,
            children: Vec::new(),
            entered: false,
        }
    }
}

/// Owns one detached child until its parent is ready to receive it.
struct ChildEntry {
    location: OwnedLocation,
    depth: usize,
    value: Value,
}

/// Detaches immediate children in reverse order, preserving traversal order.
fn take_children(value: &mut Value, depth: usize) -> Vec<ChildEntry> {
    let child_depth = depth.checked_add(1).expect(
        "a materialized JSON tree cannot have usize::MAX nesting depth",
    );
    match value {
        Value::Array(values) => std::mem::take(values)
            .into_iter()
            .enumerate()
            .rev()
            .map(|(index, value)| ChildEntry {
                location: OwnedLocation::ArrayElement(index),
                depth: child_depth,
                value,
            })
            .collect(),
        Value::Object(entries) => std::mem::take(entries)
            .into_iter()
            .rev()
            .map(|(key, value)| ChildEntry {
                location: OwnedLocation::ObjectValue(key),
                depth: child_depth,
                value,
            })
            .collect(),
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {
            Vec::new()
        }
    }
}

/// Inserts a completed child into its detached parent.
fn insert_child(parent: &mut Value, location: OwnedLocation, value: Value) {
    match (parent, location) {
        (Value::Array(values), OwnedLocation::ArrayElement(_)) => {
            values.push(value);
        }
        (Value::Object(entries), OwnedLocation::ObjectValue(key)) => {
            entries.insert(key, value);
        }
        _ => unreachable!("mutable JSON frame remains synchronized"),
    }
}

/// Reassembles detached tree parts if traversal exits with an error or panic.
struct RootRestoreGuard<'a> {
    root: &'a mut Value,
    stack: Vec<MutFrame>,
    finished: bool,
}

impl<'a> RootRestoreGuard<'a> {
    fn new(root: &'a mut Value) -> Self {
        Self {
            root,
            stack: Vec::new(),
            finished: false,
        }
    }

    fn finish(&mut self, value: Value) {
        *self.root = value;
        self.finished = true;
    }
}

impl Drop for RootRestoreGuard<'_> {
    fn drop(&mut self) {
        if self.finished || self.stack.is_empty() {
            return;
        }
        let mut completed = None;
        while let Some(mut frame) = self.stack.pop() {
            if let Some((location, value)) = completed.take() {
                insert_child(&mut frame.value, location, value);
            }
            while let Some(child) = frame.children.pop() {
                insert_child(&mut frame.value, child.location, child.value);
            }
            completed = Some((frame.location, frame.value));
        }
        let (_, value) = completed.expect("root frame exists");
        *self.root = value;
    }
}
