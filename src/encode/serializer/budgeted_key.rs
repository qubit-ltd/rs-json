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
pub(super) struct BudgetedKey<
    'a,
    'transaction,
    'budget,
    'context,
    T,
    R,
    Q,
    const VALUE_LIMITS: bool,
> where
    T: ?Sized,
    Q: ResourceQuantity,
{
    /// Original map key.
    value: &'a T,

    /// Shared traversal context.
    context: &'context RefCell<JsonEncodeContext<'transaction, 'budget, R, Q>>,
}

impl<'a, 'transaction, 'budget, 'context, T, R, Q, const VALUE_LIMITS: bool>
    BudgetedKey<'a, 'transaction, 'budget, 'context, T, R, Q, VALUE_LIMITS>
where
    T: ?Sized,
    Q: ResourceQuantity,
{
    /// Creates a key wrapper bound to the shared traversal context.
    #[inline(always)]
    pub(super) const fn new(
        value: &'a T,
        context: &'context RefCell<JsonEncodeContext<'transaction, 'budget, R, Q>>,
    ) -> Self {
        Self { value, context }
    }
}

impl<T, R, Q, const VALUE_LIMITS: bool> Serialize
    for BudgetedKey<'_, '_, '_, '_, T, R, Q, VALUE_LIMITS>
where
    T: Serialize + ?Sized,
    R: Clone,
    Q: ResourceQuantity,
{
    /// Serializes the original key once through a key-aware decorator.
    #[inline(always)]
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.value
            .serialize(JsonKeyBudgetSerializer::<S, R, Q, VALUE_LIMITS> {
                inner: serializer,
                context: self.context,
            })
    }
}
