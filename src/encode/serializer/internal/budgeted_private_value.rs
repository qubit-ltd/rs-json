// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Strict serde_json RawValue serializer adapter.

use std::cell::RefCell;

use qubit_budget::ResourceQuantity;
use serde::Serialize;
use serde::Serializer;

use super::JsonEncodeContext;
use super::JsonPrivateTextSerializer;
use super::PrivateTextKind;

/// Wraps a serde_json private string payload with budget accounting.
///
/// # Type Parameters
///
/// * `T` - Private payload type supplied by serde_json.
/// * `R` - Resource identifier used by the encode budget.
/// * `Q` - Quantity representation used by the encode budget.
pub(in crate::encode) struct BudgetedPrivateValue<'a, 'transaction, 'budget, 'context, T, R, Q>
where
    T: ?Sized,
    Q: ResourceQuantity,
{
    /// Private string payload supplied by serde_json.
    value: &'a T,

    /// Shared traversal context.
    context: &'context RefCell<JsonEncodeContext<'transaction, 'budget, R, Q>>,

    /// Budget semantics represented by the private string payload.
    kind: PrivateTextKind,
}

impl<'a, 'transaction, 'budget, 'context, T, R, Q> BudgetedPrivateValue<'a, 'transaction, 'budget, 'context, T, R, Q>
where
    T: ?Sized,
    Q: ResourceQuantity,
{
    /// Creates a private raw JSON payload wrapper at its final depth.
    ///
    /// # Parameters
    ///
    /// * `value` - Raw JSON payload provided by serde_json.
    /// * `context` - Shared encode traversal and budget state.
    /// * `depth` - Final root-inclusive depth of the payload.
    ///
    /// # Returns
    ///
    /// A wrapper that applies raw JSON preflight and budget accounting.
    #[must_use]
    #[inline]
    pub(super) const fn raw_value(
        value: &'a T,
        context: &'context RefCell<JsonEncodeContext<'transaction, 'budget, R, Q>>,
        depth: usize,
    ) -> Self {
        Self {
            value,
            context,
            kind: PrivateTextKind::RawValue { depth },
        }
    }

    /// Creates a private number payload wrapper at its final depth.
    ///
    /// # Parameters
    ///
    /// * `value` - Number payload provided by serde_json.
    /// * `context` - Shared encode traversal and budget state.
    /// * `depth` - Final root-inclusive depth of the payload.
    ///
    /// # Returns
    ///
    /// A wrapper that applies number-text preflight and budget accounting.
    #[must_use]
    #[inline]
    pub(super) const fn number(
        value: &'a T,
        context: &'context RefCell<JsonEncodeContext<'transaction, 'budget, R, Q>>,
        depth: usize,
    ) -> Self {
        Self {
            value,
            context,
            kind: PrivateTextKind::Number { depth },
        }
    }
}

impl<T, R, Q> Serialize for BudgetedPrivateValue<'_, '_, '_, '_, T, R, Q>
where
    T: Serialize + ?Sized,
    R: Clone,
    Q: ResourceQuantity,
{
    /// Traverses the private payload once through its text decorator.
    ///
    /// # Type Parameters
    ///
    /// * `S` - Serializer receiving the decorated private payload.
    ///
    /// # Parameters
    ///
    /// * `self` - Wrapper containing the private text and shared context.
    /// * `serializer` - Serializer used to encode the payload.
    ///
    /// # Returns
    ///
    /// The wrapped serializer's success value.
    ///
    /// # Errors
    ///
    /// Returns errors from private-text preflight or the wrapped serializer.
    /// An operation-level opaque failure is also recorded when an enclosing
    /// custom implementation ignores the returned error.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let result = self.value.serialize(JsonPrivateTextSerializer {
            inner: serializer,
            context: self.context,
            kind: self.kind,
        });
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
