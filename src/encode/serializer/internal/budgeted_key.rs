// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Strict Serde serializer decorator for JSON object keys.

use std::cell::RefCell;

use qubit_budget::ResourceQuantity;
use serde::Serialize;
use serde::Serializer;

use super::JsonEncodeContext;
use super::JsonKeyBudgetSerializer;

/// Wraps a map key so it is traversed once through a key-aware decorator.
///
/// # Type Parameters
///
/// * `T` - Map key type to serialize.
/// * `R` - Resource identifier used by the shared encode budget.
/// * `Q` - Quantity representation used by the shared encode budget.
/// * `VALUE_LIMITS` - Whether value-level limits apply to the key.
pub(in crate::encode) struct BudgetedKey<'a, 'transaction, 'budget, 'context, T, R, Q, const VALUE_LIMITS: bool>
where
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
    ///
    /// # Parameters
    ///
    /// * `value` - Map key whose serialization will be decorated.
    /// * `context` - Shared traversal context for key accounting.
    ///
    /// # Returns
    ///
    /// A wrapper that serializes the key through the key-aware decorator.
    #[must_use]
    #[inline(always)]
    pub(super) const fn new(
        value: &'a T,
        context: &'context RefCell<JsonEncodeContext<'transaction, 'budget, R, Q>>,
    ) -> Self {
        Self { value, context }
    }
}

impl<T, R, Q, const VALUE_LIMITS: bool> Serialize for BudgetedKey<'_, '_, '_, '_, T, R, Q, VALUE_LIMITS>
where
    T: Serialize + ?Sized,
    R: Clone,
    Q: ResourceQuantity,
{
    /// Serializes the original key once through a key-aware decorator.
    ///
    /// # Type Parameters
    ///
    /// * `S` - Serializer receiving the decorated key.
    ///
    /// # Parameters
    ///
    /// * `self` - Key wrapper containing the original key and traversal
    ///   context.
    /// * `serializer` - Serializer used to encode the key.
    ///
    /// # Returns
    ///
    /// The wrapped serializer's success value.
    ///
    /// # Errors
    ///
    /// Returns errors produced while the wrapped serializer encodes the key.
    #[inline(always)]
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.value.serialize(JsonKeyBudgetSerializer::<S, R, Q, VALUE_LIMITS> {
            inner: serializer,
            context: self.context,
        })
    }
}
