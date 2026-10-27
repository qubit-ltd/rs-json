// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! A JSON value whose owned-tree operations use explicit work stacks.

#[cfg(test)]
use std::cell::Cell;
use std::fmt;
use std::mem::ManuallyDrop;

use serde_json::Value;

use super::internal::SafeValueBuilder;

/// An owned JSON value whose clone, equality, debug formatting, and drop use
/// protected tree operations.
///
/// This protection does not extend to arbitrary recursive operations on a
/// borrowed or extracted [`serde_json::Value`]. [`as_value`](Self::as_value)
/// returns a shared borrow tied to this owner; callers remain responsible for
/// the stack behavior of recursive operations they invoke through that borrow.
/// [`into_value`](Self::into_value) gives up the protected owner and transfers
/// responsibility for the resulting tree's lifetime to the caller.
///
/// Serde interoperability uses `stacker` and still depends on the concrete
/// serializer or deserializer, which may impose its own recursion limits. The
/// owner does not enforce an exact heap-memory limit.
///
/// # Example
///
/// ```
/// use qubit_json::value::SafeJsonValue;
/// use serde_json::json;
///
/// let owner = SafeJsonValue::new(json!({"items": [1, 2]}));
/// let copy = owner.clone();
/// assert_eq!(owner, copy);
/// assert_eq!(owner.as_value()["items"][0], 1);
/// let shallow_raw = copy.into_value();
/// assert!(shallow_raw.is_object());
/// ```
pub struct SafeJsonValue {
    /// The root is optional so protected operations can take it before cleanup.
    root: Option<Value>,
}

impl SafeJsonValue {
    /// Takes ownership of a JSON value.
    #[must_use]
    pub fn new(value: Value) -> Self {
        Self { root: Some(value) }
    }

    /// Copies a JSON value using an explicit traversal stack.
    #[must_use]
    pub fn copy_from(value: &Value) -> Self {
        enum Task<'a> {
            Visit(&'a Value),
            Array(usize),
            Object(Vec<String>),
        }
        let mut tasks = vec![Task::Visit(value)];
        let mut output: Vec<Self> = Vec::new();
        while let Some(task) = tasks.pop() {
            match task {
                Task::Visit(Value::Array(values)) => {
                    tasks.push(Task::Array(values.len()));
                    tasks.extend(values.iter().rev().map(Task::Visit));
                }
                Task::Visit(Value::Object(values)) => {
                    tasks.push(Task::Object(values.keys().cloned().collect()));
                    tasks.extend(values.values().rev().map(Task::Visit));
                }
                Task::Visit(value) => output.push(Self::new(value.clone())),
                Task::Array(length) => {
                    let children = output.split_off(output.len() - length);
                    let mut builder = SafeValueBuilder::array();
                    for child in children {
                        builder.push(child);
                    }
                    output.push(Self::new(builder.finish()));
                }
                Task::Object(keys) => {
                    let children = output.split_off(output.len() - keys.len());
                    let mut builder = SafeValueBuilder::object();
                    for (key, child) in keys.into_iter().zip(children) {
                        builder.insert(key, child);
                    }
                    output.push(Self::new(builder.finish()));
                }
            }
        }
        output.pop().expect("a JSON value always produces a root")
    }

    /// Borrows the underlying value for interoperability.
    #[must_use]
    pub fn as_value(&self) -> &Value {
        self.root.as_ref().expect("owner root is present")
    }

    /// Transfers the underlying value to an unprotected caller-owned tree.
    #[must_use]
    pub fn into_value(self) -> Value {
        let mut this = ManuallyDrop::new(self);
        this.root.take().expect("owner root is present")
    }
}

impl Clone for SafeJsonValue {
    fn clone(&self) -> Self {
        Self::copy_from(self.as_value())
    }
}

impl PartialEq for SafeJsonValue {
    fn eq(&self, other: &Self) -> bool {
        let mut pending = vec![(self.as_value(), other.as_value())];
        while let Some((left, right)) = pending.pop() {
            match (left, right) {
                (Value::Null, Value::Null) => {}
                (Value::Bool(a), Value::Bool(b)) if a == b => {}
                (Value::Number(a), Value::Number(b)) if a == b => {}
                (Value::String(a), Value::String(b)) if a == b => {}
                (Value::Array(a), Value::Array(b)) if a.len() == b.len() => {
                    pending.extend(a.iter().zip(b));
                }
                (Value::Object(a), Value::Object(b)) if a.len() == b.len() => {
                    for (key, value) in a {
                        let Some(other_value) = b.get(key) else {
                            return false;
                        };
                        pending.push((value, other_value));
                    }
                }
                _ => return false,
            }
        }
        true
    }
}

impl Eq for SafeJsonValue {}

impl fmt::Debug for SafeJsonValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        enum Action<'a> {
            Value(&'a Value),
            Text(&'static str),
            Key(&'a str),
        }
        let mut actions = vec![Action::Value(self.as_value())];
        while let Some(action) = actions.pop() {
            match action {
                Action::Text(text) => f.write_str(text)?,
                Action::Key(key) => fmt::Debug::fmt(key, f)?,
                Action::Value(Value::Null) => f.write_str("Null")?,
                Action::Value(Value::Bool(value)) => write!(f, "Bool({value:?})")?,
                Action::Value(Value::Number(value)) => write!(f, "Number({value:?})")?,
                Action::Value(Value::String(value)) => write!(f, "String({value:?})")?,
                Action::Value(Value::Array(values)) => {
                    f.write_str("Array [")?;
                    actions.push(Action::Text("]"));
                    for (index, child) in values.iter().enumerate().rev() {
                        if index + 1 < values.len() {
                            actions.push(Action::Text(", "));
                        }
                        actions.push(Action::Value(child));
                    }
                }
                Action::Value(Value::Object(values)) => {
                    f.write_str("Object {")?;
                    actions.push(Action::Text("}"));
                    for (index, (key, child)) in values.iter().enumerate().rev() {
                        if index + 1 < values.len() {
                            actions.push(Action::Text(", "));
                        }
                        actions.push(Action::Value(child));
                        actions.push(Action::Text(": "));
                        actions.push(Action::Key(key));
                    }
                }
            }
        }
        Ok(())
    }
}

impl Drop for SafeJsonValue {
    fn drop(&mut self) {
        if let Some(mut root) = self.root.take() {
            drop_tree(&mut root);
        }
    }
}

/// Drops a value tree iteratively, falling back to allocation-free traversal.
pub(super) fn drop_tree(root: &mut Value) {
    let mut pending = Vec::new();
    if !try_reserve_drop_work(&mut pending, 1) {
        drop_tree_slow(root);
        return;
    }
    pending.push(std::mem::replace(root, Value::Null));
    while let Some(value) = pending.pop() {
        match value {
            Value::Array(mut values) => {
                if !try_reserve_drop_work(&mut pending, values.len()) {
                    for child in &mut values {
                        drop_tree_slow(child);
                    }
                    for child in &mut pending {
                        drop_tree_slow(child);
                    }
                    return;
                }
                pending.append(&mut values);
            }
            Value::Object(mut values) => {
                if !try_reserve_drop_work(&mut pending, values.len()) {
                    for child in values.values_mut() {
                        drop_tree_slow(child);
                    }
                    for child in &mut pending {
                        drop_tree_slow(child);
                    }
                    return;
                }
                pending.extend(values.into_values());
            }
            shallow => drop(shallow),
        }
    }
}

/// Attempts to grow the explicit cleanup stack without panicking on allocation
/// failure.
fn try_reserve_drop_work(pending: &mut Vec<Value>, additional: usize) -> bool {
    #[cfg(test)]
    if FAIL_DROP_RESERVE_AFTER.with(|remaining| match remaining.get() {
        Some(0) => {
            remaining.set(None);
            true
        }
        Some(count) => {
            remaining.set(Some(count - 1));
            false
        }
        None => false,
    }) {
        return false;
    }
    pending.try_reserve(additional).is_ok()
}

#[cfg(test)]
thread_local! {
    static FAIL_DROP_RESERVE_AFTER: Cell<Option<usize>> = const { Cell::new(None) };
    static PANIC_AFTER_INSERT: Cell<Option<usize>> = const { Cell::new(None) };
}

#[cfg(test)]
impl SafeJsonValue {
    pub(crate) fn fail_drop_reserve_after_for_test(successful_reserves: usize) {
        FAIL_DROP_RESERVE_AFTER.with(|remaining| remaining.set(Some(successful_reserves)));
    }

    pub(crate) fn assert_drop_reserve_failure_consumed_for_test() {
        assert!(FAIL_DROP_RESERVE_AFTER.with(Cell::get).is_none());
    }

    pub(crate) fn panic_after_guarded_inserts_for_test(count: usize) {
        PANIC_AFTER_INSERT.with(|remaining| remaining.set(Some(count)));
    }

    pub(crate) fn assert_guarded_insert_panic_consumed_for_test() {
        assert!(PANIC_AFTER_INSERT.with(|remaining| remaining.get().is_none()));
    }
}

/// Injects an unwind after guarded inserts in tests; this is a no-op in builds.
pub(super) fn maybe_panic_after_guarded_insert() {
    #[cfg(test)]
    PANIC_AFTER_INSERT.with(|remaining| {
        if let Some(count) = remaining.get() {
            if count <= 1 {
                remaining.set(None);
                panic!("injected unwind while building a protected JSON value");
            }
            remaining.set(Some(count - 1));
        }
    });
}

/// Cleans a tree without allocating by repeatedly removing leaves in place.
fn drop_tree_slow(root: &mut Value) {
    loop {
        let mut current = std::ptr::from_mut(root);
        loop {
            // SAFETY: `current` always points into `root`; no collection is
            // structurally changed while its child pointer is being followed.
            let value = unsafe { &mut *current };
            let child = match value {
                Value::Array(values) => {
                    values.retain(|value| !value.is_null());
                    values.first_mut().map(std::ptr::from_mut)
                }
                Value::Object(values) => {
                    values.retain(|_, value| !value.is_null());
                    values.values_mut().next().map(std::ptr::from_mut)
                }
                _ => None,
            };
            if let Some(child) = child {
                current = child;
                continue;
            }
            // SAFETY: `current` was obtained from the current tree and points
            // at a leaf or empty container, whose destruction is shallow.
            let old = unsafe { std::mem::replace(&mut *current, Value::Null) };
            drop(old);
            break;
        }
        if root.is_null() {
            break;
        }
    }
}
