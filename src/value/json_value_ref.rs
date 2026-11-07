// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use std::fmt;

use serde::Serialize;
use serde::Serializer;
use serde_json::Value;

use super::internal::debug_value;
use super::internal::values_equal;

/// An immutable, borrowed view of a JSON value.
///
/// Constructing a view copies only a reference. Equality and debug formatting
/// use explicit work stacks, which may allocate. Serialization uses the same
/// `stacker`-backed recursive Serde bridge as
/// [`SafeJsonValue`](super::SafeJsonValue). The bridge can grow the stack for
/// library-side recursive calls, but the selected serializer may still impose
/// its own recursion limits; it does not promise an exact memory bound or
/// support unlimited depth. The view does not protect the borrowed tree's
/// destruction when its owner is dropped.
///
/// # Examples
///
/// ```
/// use qubit_json::value::{JsonValueRef, SafeJsonValue};
///
/// let owner = SafeJsonValue::new(serde_json::json!({"items": [1, 2]}));
/// let view = JsonValueRef::new(owner.as_value());
/// assert_eq!(
///     serde_json::to_vec(&view).unwrap(),
///     serde_json::to_vec(&owner).unwrap()
/// );
/// ```
///
/// The view cannot outlive its borrowed owner:
///
/// ```compile_fail
/// use qubit_json::value::{JsonValueRef, SafeJsonValue};
/// let view = {
///     let owner = SafeJsonValue::new(serde_json::json!([1]));
///     JsonValueRef::new(owner.as_value())
/// };
/// println!("{view:?}");
/// ```
#[derive(Clone, Copy)]
pub struct JsonValueRef<'a> {
    /// The borrowed tree; its owner must outlive this view.
    value: &'a Value,
}

impl<'a> JsonValueRef<'a> {
    /// Borrows a JSON tree without copying, traversing, or admitting it to a
    /// budget.
    ///
    /// # Parameters
    ///
    /// * `value` - Tree that must remain alive for the lifetime of this view.
    ///
    /// # Returns
    ///
    /// A read-only view tied to the supplied borrow. Construction does not
    /// allocate.
    #[must_use]
    #[inline]
    pub const fn new(value: &'a Value) -> Self {
        Self { value }
    }

    /// Returns the borrowed JSON value, preserving its original owner lifetime.
    ///
    /// Consuming this view does not extend the lifetime of the underlying tree.
    ///
    /// # Returns
    ///
    /// The JSON tree borrowed by this view.
    #[must_use]
    #[inline]
    pub const fn as_value(self) -> &'a Value {
        self.value
    }
}

impl<'a, 'b> PartialEq<JsonValueRef<'b>> for JsonValueRef<'a> {
    /// Compares the referenced JSON trees without requiring matching borrow
    /// lifetimes.
    ///
    /// Equality uses an explicit work stack, which may allocate for nested
    /// values.
    ///
    /// # Parameters
    ///
    /// * `other` - View of the JSON tree to compare with this one.
    ///
    /// # Returns
    ///
    /// `true` when both referenced trees are equal; otherwise, `false`.
    fn eq(&self, other: &JsonValueRef<'b>) -> bool {
        values_equal(self.value, other.value)
    }
}

impl Eq for JsonValueRef<'_> {}

impl fmt::Debug for JsonValueRef<'_> {
    /// Formats the referenced JSON tree using the safe recursive formatting
    /// bridge.
    ///
    /// Formatting may allocate an explicit work stack for nested values.
    ///
    /// # Parameters
    ///
    /// * `formatter` - Formatter that receives the JSON tree's debug
    ///   representation.
    ///
    /// # Returns
    ///
    /// `Ok(())` when formatting succeeds, or the formatting error reported by
    /// the formatter.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        debug_value(self.value, formatter)
    }
}

impl Serialize for JsonValueRef<'_> {
    /// Serializes the referenced JSON tree through the stack-growing Serde
    /// bridge.
    ///
    /// The bridge supports library-side recursive calls, while the serializer
    /// may still impose its own recursion limits.
    ///
    /// # Type Parameters
    ///
    /// * `S` - Serializer that receives the JSON tree's serialized
    ///   representation.
    ///
    /// # Parameters
    ///
    /// * `serializer` - Serializer used to encode the borrowed JSON tree.
    ///
    /// # Returns
    ///
    /// The value produced by the serializer after encoding succeeds.
    ///
    /// # Errors
    ///
    /// Returns the serializer's error if encoding the tree fails.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        super::safe_json_serde::serialize_value(self.value, serializer)
    }
}
