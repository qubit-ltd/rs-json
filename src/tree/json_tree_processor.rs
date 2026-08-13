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
    pub fn process<V>(
        &mut self,
        value: &'a Value,
        visitor: &mut V,
    ) -> Result<(), JsonTreeProcessError<R, Q, V::Error>>
    where
        V: JsonTreeVisitor,
    {
        let mut pending = vec![Frame::Enter {
            value,
            context: JsonTreeContext {
                depth: 1,
                location: JsonTreeLocation::Root,
            },
            key_to_charge: None,
        }];
        while let Some(frame) = pending.pop() {
            match frame {
                Frame::Enter {
                    value,
                    context,
                    key_to_charge,
                } => {
                    if let Some(key) = key_to_charge {
                        self.budget.consume_key_bytes_usize(key.len())?;
                    }
                    self.admit(value, context.depth)?;
                    visitor
                        .enter(value, context)
                        .map_err(JsonTreeProcessError::Visitor)?;
                    pending.push(Frame::Leave { value, context });
                    self.push_children(value, context.depth, &mut pending);
                }
                Frame::Leave { value, context } => visitor
                    .leave(value, context)
                    .map_err(JsonTreeProcessError::Visitor)?,
            }
        }
        Ok(())
    }

    /// Mutates every admitted node in depth-first order without Rust recursion.
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
                guard.stack[index].active = Some(child.location.clone());
                guard.stack.push(MutFrame::child(child));
                continue;
            }

            let frame = guard.stack.pop().expect("mutable frame exists");
            if let Some(parent) = guard.stack.last_mut() {
                let location = parent
                    .active
                    .take()
                    .expect("completed child has an active parent slot");
                insert_child(&mut parent.value, location, frame.value);
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

    /// Pushes descendants in reverse order so stack popping preserves JSON
    /// order.
    fn push_children(
        &self,
        value: &'a Value,
        depth: usize,
        pending: &mut Vec<Frame<'a>>,
    ) {
        let child_depth = depth.checked_add(1).expect(
            "a materialized JSON tree cannot have usize::MAX nesting depth",
        );
        match value {
            Value::Array(values) => {
                for (index, child) in values.iter().enumerate().rev() {
                    pending.push(Frame::Enter {
                        value: child,
                        context: JsonTreeContext {
                            depth: child_depth,
                            location: JsonTreeLocation::ArrayElement { index },
                        },
                        key_to_charge: None,
                    });
                }
            }
            Value::Object(entries) => {
                for (key, child) in entries.iter().rev() {
                    pending.push(Frame::Enter {
                        value: child,
                        context: JsonTreeContext {
                            depth: child_depth,
                            location: JsonTreeLocation::ObjectValue { key },
                        },
                        key_to_charge: Some(key),
                    });
                }
            }
            Value::Null
            | Value::Bool(_)
            | Value::Number(_)
            | Value::String(_) => {}
        }
    }
}

/// Represents one delayed enter or leave operation in a depth-first traversal.
enum Frame<'a> {
    /// Enters a node after optionally charging its object key.
    Enter {
        value: &'a Value,
        context: JsonTreeContext<'a>,
        key_to_charge: Option<&'a str>,
    },
    /// Leaves a node after its descendants are complete.
    Leave {
        value: &'a Value,
        context: JsonTreeContext<'a>,
    },
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
    active: Option<OwnedLocation>,
    entered: bool,
}

impl MutFrame {
    fn root(value: Value) -> Self {
        Self {
            location: OwnedLocation::Root,
            depth: 1,
            value,
            children: Vec::new(),
            active: None,
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
            active: None,
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

/// Restores the original root if traversal exits with an error or panic.
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
            if let Some(value) = completed.take() {
                let location = frame
                    .active
                    .take()
                    .expect("completed child has an active parent slot");
                insert_child(&mut frame.value, location, value);
            }
            while let Some(child) = frame.children.pop() {
                insert_child(&mut frame.value, child.location, child.value);
            }
            completed = Some(frame.value);
        }
        *self.root = completed.expect("root frame exists");
    }
}
