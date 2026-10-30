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

use super::internal::JsonValueCopyFrame;
use super::internal::SafeValueBuilder;
use super::internal::debug_value;
use super::internal::values_equal;

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
/// Its deserializer accepts `i128` and `u128` events when their values fit the
/// JSON integer range, maps `None` to `null`, and transparently unwraps `Some`
/// and newtype payloads.
///
/// The tree protection and the meaning of values produced by `Deserialize`
/// are separate concerns. A direct Serde bridge cannot identify whether a
/// map-shaped event came from an ordinary object or an upstream deserializer's
/// private number representation. With `serde_json`'s `arbitrary_precision`
/// feature, number shapes produced by that bridge can therefore differ from
/// those produced without the feature. When decoding JSON text and preserving
/// its numeric meaning is important, use [`JsonDecoder::decode_value_str`]
/// (or the corresponding byte and reader methods) instead.
///
/// Direct deserialization also rejects duplicate object keys after decoding
/// their string escapes. It does not normalize Unicode. In contrast,
/// [`Self::new`] takes an existing [`serde_json::Value`] as-is: if an upstream
/// deserializer has already merged duplicate keys, this type cannot recover
/// them.
///
/// [`JsonDecoder::decode_value_str`]: crate::decode::JsonDecoder::decode_value_str
///
/// # Examples
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
/// assert!(serde_json::from_str::<SafeJsonValue>(r#"{"a":1,"\u0061":2}"#).is_err());
/// ```
pub struct SafeJsonValue {
    /// The root is optional so protected operations can take it before cleanup.
    root: Option<Value>,
}

impl SafeJsonValue {
    /// Takes ownership of a JSON tree without traversing it or admitting it to
    /// a budget.
    ///
    /// # Parameters
    ///
    /// * `value` - JSON tree whose ownership is transferred to this owner.
    ///
    /// # Returns
    ///
    /// An owner for the supplied tree. Construction does not copy or allocate.
    #[must_use]
    #[inline]
    pub fn new(value: Value) -> Self {
        Self { root: Some(value) }
    }

    /// Copies an entire JSON tree using an explicit traversal stack.
    ///
    /// This operation allocates work-stack storage and result storage for the
    /// copied tree; it does not impose a budget on the source tree.
    ///
    /// # Parameters
    ///
    /// * `value` - JSON tree to copy.
    ///
    /// # Returns
    ///
    /// An independently owned copy of the complete tree.
    #[must_use]
    pub fn copy_from(value: &Value) -> Self {
        #[cfg(test)]
        COPY_FROM_CALLS.with(|calls| calls.set(calls.get() + 1));

        let mut frames: Vec<JsonValueCopyFrame<'_>> = Vec::new();
        let mut builders: Vec<SafeValueBuilder> = Vec::new();
        let mut next_value = Some(value);
        let mut completed_value = None;
        loop {
            if let Some(source) = next_value.take() {
                match source {
                    Value::Array(values) => {
                        let mut frame = JsonValueCopyFrame::array(values);
                        if let Some(child) = frame.next_value() {
                            if frame.has_remaining() {
                                frame.start_builder(&mut builders);
                            }
                            frames.push(frame);
                            next_value = Some(child);
                        } else {
                            completed_value = Some(frame.finish_empty());
                        }
                    }
                    Value::Object(values) => {
                        let mut frame = JsonValueCopyFrame::object(values);
                        if let Some(child) = frame.next_value() {
                            if frame.has_remaining() {
                                frame.start_builder(&mut builders);
                            }
                            frames.push(frame);
                            next_value = Some(child);
                        } else {
                            completed_value = Some(frame.finish_empty());
                        }
                    }
                    leaf => completed_value = Some(Self::new(leaf.clone())),
                }
            }

            if let Some(child) = completed_value.take() {
                let Some(parent) = frames.last_mut() else {
                    return child;
                };
                if let Some(completed_parent) = parent.push_child(child, &mut builders) {
                    frames.pop().expect("a completed child has a parent frame");
                    completed_value = Some(completed_parent);
                } else if let Some(source) = parent.next_value() {
                    next_value = Some(source);
                } else {
                    completed_value = Some(
                        frames
                            .pop()
                            .expect("a completed child has a parent frame")
                            .finish(&mut builders),
                    );
                }
            }
        }
    }

    /// Returns a shared borrow of the underlying JSON tree for
    /// interoperability.
    ///
    /// The returned borrow is tied to this owner. Recursive operations
    /// performed through it are the caller's responsibility and may use the
    /// backend's stack behavior.
    ///
    /// # Returns
    ///
    /// The underlying tree borrowed for no longer than this owner.
    #[must_use]
    #[inline]
    pub fn as_value(&self) -> &Value {
        self.root.as_ref().expect("owner root is present")
    }

    /// Transfers the tree and its destruction responsibility to the caller.
    ///
    /// The returned `Value` is no longer protected by this owner; recursive
    /// operations and destruction are the caller's responsibility.
    ///
    /// # Returns
    ///
    /// The previously owned JSON tree.
    #[must_use]
    #[inline]
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
        values_equal(self.as_value(), other.as_value())
    }
}

impl Eq for SafeJsonValue {}

impl fmt::Debug for SafeJsonValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        debug_value(self.as_value(), formatter)
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
    static COPY_FROM_CALLS: Cell<usize> = const { Cell::new(0) };
    static FAIL_DROP_RESERVE_AFTER: Cell<Option<usize>> = const { Cell::new(None) };
    static PANIC_AFTER_INSERT: Cell<Option<usize>> = const { Cell::new(None) };
}

#[cfg(test)]
impl SafeJsonValue {
    pub(crate) fn take_copy_from_calls_for_test() -> usize {
        COPY_FROM_CALLS.with(|calls| calls.replace(0))
    }

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
