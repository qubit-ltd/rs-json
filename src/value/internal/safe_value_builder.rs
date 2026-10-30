// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Builds values whose partial trees retain protected cleanup.

use serde_json::Value;

use super::super::SafeJsonValue;

/// Owns a partially built tree and drops it with the owner's iterative cleanup.
pub(crate) struct SafeValueBuilder {
    /// `None` only after `finish` transfers the completed tree.
    value: Option<Value>,
}

impl SafeValueBuilder {
    /// Starts an empty array builder.
    pub(crate) fn array() -> Self {
        Self {
            value: Some(Value::Array(Vec::new())),
        }
    }

    /// Starts an empty array builder with a caller-bounded allocation hint.
    pub(crate) fn array_with_capacity(capacity: usize) -> Self {
        Self {
            value: Some(Value::Array(Vec::with_capacity(capacity))),
        }
    }

    /// Starts an empty object builder.
    pub(crate) fn object() -> Self {
        Self {
            value: Some(Value::Object(serde_json::Map::new())),
        }
    }

    /// Appends a child owner while keeping an unwind-safe placeholder in place.
    pub(crate) fn push(&mut self, child: SafeJsonValue) {
        let Value::Array(values) = self.value.as_mut().expect("builder is active") else {
            unreachable!()
        };
        values.push(Value::Null);
        *values.last_mut().expect("just inserted a slot") = child.into_value();
        super::super::safe_json_value::maybe_panic_after_guarded_insert();
    }

    /// Inserts a child owner while keeping an unwind-safe placeholder in place.
    pub(crate) fn insert(&mut self, key: String, child: SafeJsonValue) {
        let Value::Object(values) = self.value.as_mut().expect("builder is active") else {
            unreachable!()
        };
        let slot = values.entry(key).or_insert(Value::Null);
        *slot = child.into_value();
        super::super::safe_json_value::maybe_panic_after_guarded_insert();
    }

    /// Transfers the completed tree out of the guarded builder.
    pub(crate) fn finish(mut self) -> Value {
        self.value.take().expect("builder is active")
    }
}

impl Drop for SafeValueBuilder {
    fn drop(&mut self) {
        if let Some(mut value) = self.value.take() {
            super::super::safe_json_value::drop_tree(&mut value);
        }
    }
}
