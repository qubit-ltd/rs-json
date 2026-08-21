// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Strict serde_json private Number and RawValue serializer adapters.
// qubit-style: allow source-test-pair
// qubit-style: allow multiple-public-types
// qubit-style: allow explicit-imports

use std::cell::RefCell;

use qubit_budget::ResourceQuantity;
use serde::Serialize;
use serde::Serializer;

use super::internal::JsonPrivateTextSerializer;
use super::internal::PrivateTextKind;
use super::json_encode_context::JsonEncodeContext;

/// Wraps a serde_json private string payload with budget accounting.
pub(super) struct BudgetedPrivateValue<'a, 'transaction, 'budget, 'context, T, R, Q>
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
    /// Creates a private arbitrary-precision number payload wrapper.
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

    /// Creates a private raw JSON payload wrapper at its final depth.
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
}

impl<T, R, Q> Serialize for BudgetedPrivateValue<'_, '_, '_, '_, T, R, Q>
where
    T: Serialize + ?Sized,
    R: Clone,
    Q: ResourceQuantity,
{
    /// Traverses the private payload once through its text decorator.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.value.serialize(JsonPrivateTextSerializer {
            inner: serializer,
            context: self.context,
            kind: self.kind,
        })
    }
}
