// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Strict Serde serializer decorator for JSON object keys.
// qubit-style: allow source-test-pair
// qubit-style: allow multiple-public-types
// qubit-style: allow explicit-imports

use std::cell::RefCell;

use qubit_budget::ResourceQuantity;
use serde::Serialize;
use serde::Serializer;

use super::internal::JsonKeyBudgetSerializer;
use super::json_encode_context::JsonEncodeContext;

/// Wraps a map key so it is traversed once through a key-aware decorator.
pub(super) struct BudgetedKey<'a, 'transaction, 'budget, 'context, T, R, Q>
where
    T: ?Sized,
    Q: ResourceQuantity,
{
    /// Original map key.
    value: &'a T,

    /// Shared traversal context.
    context: &'context RefCell<JsonEncodeContext<'transaction, 'budget, R, Q>>,

    /// Whether key accounting can reject the emitted key.
    has_value_limits: bool,
}

impl<'a, 'transaction, 'budget, 'context, T, R, Q> BudgetedKey<'a, 'transaction, 'budget, 'context, T, R, Q>
where
    T: ?Sized,
    Q: ResourceQuantity,
{
    /// Creates a key wrapper bound to the shared traversal context.
    #[inline(always)]
    pub(super) const fn new(
        value: &'a T,
        context: &'context RefCell<JsonEncodeContext<'transaction, 'budget, R, Q>>,
        has_value_limits: bool,
    ) -> Self {
        Self {
            value,
            context,
            has_value_limits,
        }
    }
}

impl<T, R, Q> Serialize for BudgetedKey<'_, '_, '_, '_, T, R, Q>
where
    T: Serialize + ?Sized,
    R: Clone,
    Q: ResourceQuantity,
{
    /// Serializes the original key once through a key-aware decorator.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.value.serialize(JsonKeyBudgetSerializer {
            inner: serializer,
            context: self.context,
            has_value_limits: self.has_value_limits,
        })
    }
}
