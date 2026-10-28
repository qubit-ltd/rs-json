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
/// use explicit work stacks, which may allocate. Serialization delegates to
/// the selected Serde serializer and remains subject to that serializer's
/// recursion limits. The view does not protect the borrowed tree's destruction
/// when its owner is dropped.
///
/// # Example
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
    value: &'a Value,
}

impl<'a> JsonValueRef<'a> {
    /// Creates a view without copying or allocating the value tree.
    pub const fn new(value: &'a Value) -> Self {
        Self { value }
    }

    /// Returns the borrowed JSON value.
    pub const fn as_value(self) -> &'a Value {
        self.value
    }
}

impl<'a, 'b> PartialEq<JsonValueRef<'b>> for JsonValueRef<'a> {
    fn eq(&self, other: &JsonValueRef<'b>) -> bool {
        values_equal(self.value, other.value)
    }
}

impl Eq for JsonValueRef<'_> {}

impl fmt::Debug for JsonValueRef<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        debug_value(self.value, formatter)
    }
}

impl Serialize for JsonValueRef<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        super::safe_json_serde::serialize_value(self.value, serializer)
    }
}
