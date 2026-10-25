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

use super::JsonEncodeContext;
use super::JsonEncodeSerializer;

/// Re-enters the budget-aware serializer for one nested value.
///
/// The wrapped value and shared context are borrowed for the nested
/// serialization operation so all descendants contribute to the same budget.
///
/// # Type Parameters
///
/// * `T` - Nested value type.
/// * `R` - Resource identity tracked by the encoder.
/// * `Q` - Quantity representation used for resource accounting.
/// * `VALUE_LIMITS` - Whether nested value limits are enforced.
pub(in crate::encode) struct BudgetedValue<'a, 'transaction, 'budget, 'context, T, R, Q, const VALUE_LIMITS: bool>
where
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
    ///
    /// # Parameters
    ///
    /// * `value` - Nested value to serialize.
    /// * `context` - Shared mutable accounting context for this operation.
    /// * `depth` - Root-inclusive depth assigned to the nested value.
    ///
    /// # Returns
    ///
    /// A wrapper that serializes the value through the shared context.
    #[must_use]
    #[inline(always)]
    pub(super) const fn new(
        value: &'a T,
        context: &'context RefCell<JsonEncodeContext<'transaction, 'budget, R, Q>>,
        depth: usize,
    ) -> Self {
        Self { value, context, depth }
    }
}

impl<T, R, Q, const VALUE_LIMITS: bool> Serialize for BudgetedValue<'_, '_, '_, '_, T, R, Q, VALUE_LIMITS>
where
    T: Serialize + ?Sized,
    R: Clone,
    Q: ResourceQuantity,
{
    /// Serializes the wrapped value through a child budget decorator.
    ///
    /// # Type Parameters
    ///
    /// * `S` - Serializer receiving the budget-decorated nested value.
    ///
    /// # Parameters
    ///
    /// * `self` - Wrapper holding the nested value and shared budget context.
    /// * `serializer` - Downstream serializer to decorate.
    ///
    /// # Returns
    ///
    /// The downstream serializer result for the wrapped value.
    ///
    /// # Errors
    ///
    /// Returns the downstream serializer error if value serialization or a
    /// nested budget check fails. An operation-level opaque failure is also
    /// recorded when nested serialization returns an error that an enclosing
    /// custom implementation may ignore.
    #[inline(always)]
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let result = self
            .value
            .serialize(JsonEncodeSerializer::<S, R, Q, VALUE_LIMITS>::with_context(
                serializer,
                self.context,
                self.depth,
            ));
        if result.is_err() {
            self.context
                .borrow()
                .output
                .borrow_mut()
                .record_custom_serialization_error();
        }
        result
    }
}
