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
        let mut path = Vec::new();
        let mut location = OwnedLocation::Root;
        let mut pending = Some((location.clone(), false));
        let mut containers = Vec::new();
        while let Some((current_location, key_charged)) = pending.take() {
            location = current_location;
            let depth = path.len() + 1;
            let context = location.context(depth);
            if !key_charged
                && let Some(key) = location.key()
                && let Err(error) =
                    self.budget.consume_key_bytes_usize(key.len())
            {
                let value = value_at_mut(root, &path);
                match visitor
                    .reject_budget(value, context, &error)
                    .map_err(JsonTreeProcessError::Visitor)?
                {
                    JsonBudgetRejection::Abort => {
                        return Err(JsonTreeProcessError::Budget(error));
                    }
                    JsonBudgetRejection::SkipSubtree => {
                        pending =
                            next_mut_node(root, &mut path, &mut containers);
                        continue;
                    }
                }
            }
            let admission = self.admit(value_at(root, &path), depth);
            if let Err(error) = admission {
                let value = value_at_mut(root, &path);
                match visitor
                    .reject_budget(value, context, &error)
                    .map_err(JsonTreeProcessError::Visitor)?
                {
                    JsonBudgetRejection::Abort => {
                        return Err(JsonTreeProcessError::Budget(error));
                    }
                    JsonBudgetRejection::SkipSubtree => {
                        pending =
                            next_mut_node(root, &mut path, &mut containers);
                        continue;
                    }
                }
            }
            let control = visitor
                .visit(value_at_mut(root, &path), context)
                .map_err(JsonTreeProcessError::Visitor)?;
            let child_count = match control {
                JsonTreeControl::Descend => child_count(value_at(root, &path)),
                JsonTreeControl::SkipSubtree => 0,
            };
            if child_count == 0 {
                pending = next_mut_node(root, &mut path, &mut containers);
            } else {
                containers.push(MutContainerFrame {
                    path_len: path.len(),
                    next_child: 1,
                    child_count,
                });
                let child_location = child_location(value_at(root, &path), 0);
                path.push(child_location.segment());
                pending = Some((child_location, false));
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

    /// Converts this location into a path segment.
    fn segment(&self) -> PathSegment {
        match self {
            Self::Root => {
                unreachable!("the root cannot be a child path segment")
            }
            Self::ArrayElement(index) => PathSegment::ArrayElement(*index),
            Self::ObjectValue(key) => PathSegment::ObjectValue(key.clone()),
        }
    }
}

/// Selects one child while resolving a mutable path from the root on demand.
enum PathSegment {
    /// An array child identified by its zero-based index.
    ArrayElement(usize),
    /// An object child identified by its key.
    ObjectValue(String),
}

/// Stores continuation state for one mutable container without retaining
/// aliases.
struct MutContainerFrame {
    /// Number of path elements used to reach this container.
    path_len: usize,
    /// Next child ordinal not yet processed.
    next_child: usize,
    /// Number of children observed after the parent visitor completed.
    child_count: usize,
}

/// Returns the JSON value selected by an internally maintained path.
fn value_at<'a>(mut value: &'a Value, path: &[PathSegment]) -> &'a Value {
    for segment in path {
        value = match (value, segment) {
            (Value::Array(values), PathSegment::ArrayElement(index)) => {
                &values[*index]
            }
            (Value::Object(values), PathSegment::ObjectValue(key)) => {
                &values[key]
            }
            _ => unreachable!(
                "mutable JSON traversal path remains synchronized with the tree"
            ),
        };
    }
    value
}

/// Returns the mutable JSON value selected by an internally maintained path.
fn value_at_mut<'a>(
    mut value: &'a mut Value,
    path: &[PathSegment],
) -> &'a mut Value {
    for segment in path {
        value = match (value, segment) {
            (Value::Array(values), PathSegment::ArrayElement(index)) => {
                &mut values[*index]
            }
            (Value::Object(values), PathSegment::ObjectValue(key)) => {
                &mut values[key]
            }
            _ => unreachable!(
                "mutable JSON traversal path remains synchronized with the tree"
            ),
        };
    }
    value
}

/// Counts the immediate descendants of a JSON value.
fn child_count(value: &Value) -> usize {
    match value {
        Value::Array(values) => values.len(),
        Value::Object(values) => values.len(),
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => 0,
    }
}

/// Returns the owned location of one child in iteration order.
fn child_location(value: &Value, ordinal: usize) -> OwnedLocation {
    match value {
        Value::Array(_) => OwnedLocation::ArrayElement(ordinal),
        Value::Object(values) => OwnedLocation::ObjectValue(
            values
                .keys()
                .nth(ordinal)
                .expect("stored object child ordinal remains in bounds")
                .clone(),
        ),
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {
            unreachable!("scalar JSON values have no children")
        }
    }
}

/// Advances a mutable traversal to the next child or finishes exhausted frames.
fn next_mut_node(
    root: &Value,
    path: &mut Vec<PathSegment>,
    containers: &mut Vec<MutContainerFrame>,
) -> Option<(OwnedLocation, bool)> {
    while let Some(frame) = containers.last_mut() {
        if frame.next_child < frame.child_count {
            let ordinal = frame.next_child;
            frame.next_child += 1;
            path.truncate(frame.path_len);
            let location = child_location(value_at(root, path), ordinal);
            path.push(location.segment());
            return Some((location, false));
        }
        path.truncate(frame.path_len);
        containers.pop();
    }
    None
}
