// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Nested value wrapper for strict budget-aware JSON encoding.

use std::cell::RefCell;

use qubit_budget::ResourceQuantity;
use serde::Serialize;
use serde::Serializer;

use super::json_encode_context::JsonEncodeContext;
use super::json_encode_serializer::JsonEncodeSerializer;

/// Re-enters the budget-aware serializer for one nested value.
pub(super) struct BudgetedValue<
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
    /// Original nested value.
    value: &'a T,

    /// Shared mutable budget state for the serialization traversal.
    context: &'context RefCell<JsonEncodeContext<'transaction, 'budget, R, Q>>,

    /// Root-inclusive depth assigned to the nested value.
    depth: usize,
}

impl<'a, 'transaction, 'budget, 'context, T, R, Q, const VALUE_LIMITS: bool>
    BudgetedValue<'a, 'transaction, 'budget, 'context, T, R, Q, VALUE_LIMITS>
where
    T: ?Sized,
    Q: ResourceQuantity,
{
    /// Creates a nested value wrapper bound to a shared budget context.
    #[inline(always)]
    pub(super) const fn new(
        value: &'a T,
        context: &'context RefCell<JsonEncodeContext<'transaction, 'budget, R, Q>>,
        depth: usize,
    ) -> Self {
        Self {
            value,
            context,
            depth,
        }
    }
}

impl<T, R, Q, const VALUE_LIMITS: bool> Serialize
    for BudgetedValue<'_, '_, '_, '_, T, R, Q, VALUE_LIMITS>
where
    T: Serialize + ?Sized,
    R: Clone,
    Q: ResourceQuantity,
{
    /// Serializes the wrapped value through a child budget decorator.
    #[inline(always)]
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.value
            .serialize(JsonEncodeSerializer::<S, R, Q, VALUE_LIMITS>::with_context(
                serializer,
                self.context,
                self.depth,
            ))
    }
}
