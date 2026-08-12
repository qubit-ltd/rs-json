// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tracks mutable accounting for one JSON decoding operation.

use qubit_budget::BudgetError;
use qubit_budget::MeasuredBudgetError;
use qubit_budget::ResourceBudget;
use qubit_budget::ResourceQuantity;

use super::JsonDecodeLimits;
use super::JsonResource;
use super::JsonValueBudget;

enum DecodeStorage<'a, R, Q>
where
    Q: ResourceQuantity,
{
    Owned {
        input: Option<ResourceBudget<R, Q>>,
        value: JsonValueBudget<R, Q>,
    },
    Borrowed {
        input: Option<&'a mut ResourceBudget<R, Q>>,
        value: &'a mut JsonValueBudget<R, Q>,
    },
}

/// Mutable state for one JSON decoding operation.
#[must_use]
#[derive(Debug)]
pub struct JsonDecodeSession<'a, R = JsonResource, Q = usize>
where
    Q: ResourceQuantity,
{
    storage: DecodeStorage<'a, R, Q>,
}

impl<'a, R, Q> JsonDecodeSession<'a, R, Q>
where
    R: Clone,
    Q: ResourceQuantity,
{
    /// Creates a session borrowing caller-owned input and value budgets.
    #[inline]
    pub fn borrowing(
        input: Option<&'a mut ResourceBudget<R, Q>>,
        value: &'a mut JsonValueBudget<R, Q>,
    ) -> Self {
        Self {
            storage: DecodeStorage::Borrowed { input, value },
        }
    }

    /// Consumes input bytes atomically for this decoding operation.
    #[inline]
    pub fn consume_input_bytes(
        &mut self,
        amount: Q,
    ) -> Result<(), BudgetError<R, Q>> {
        match &mut self.storage {
            DecodeStorage::Owned { input, .. } => match input {
                Some(input) => input.try_consume(amount),
                None => Ok(()),
            },
            DecodeStorage::Borrowed { input, .. } => match input {
                Some(input) => input.try_consume(amount),
                None => Ok(()),
            },
        }
    }

    /// Converts and consumes native input bytes atomically.
    ///
    /// Conversion is skipped when input-byte accounting is not configured.
    #[inline]
    pub fn consume_input_bytes_usize(
        &mut self,
        amount: usize,
    ) -> Result<(), MeasuredBudgetError<R, Q>> {
        let input = match &mut self.storage {
            DecodeStorage::Owned { input, .. } => input.as_mut(),
            DecodeStorage::Borrowed { input, .. } => input.as_deref_mut(),
        };
        let Some(input) = input else {
            return Ok(());
        };
        let amount = Q::try_from_usize(amount).map_err(|source| {
            MeasuredBudgetError::quantity(input.resource().clone(), source)
        })?;
        input.try_consume(amount).map_err(MeasuredBudgetError::from)
    }

    /// Returns the configured cumulative input-byte maximum.
    #[must_use]
    #[inline]
    pub fn max_input_bytes(&self) -> Option<Q> {
        match &self.storage {
            DecodeStorage::Owned { input, .. } => {
                input.as_ref().map(|budget| budget.limit())
            }
            DecodeStorage::Borrowed { input, .. } => {
                input.as_ref().map(|budget| budget.limit())
            }
        }
    }

    /// Returns the input budget when input accounting is configured.
    #[inline]
    pub const fn input_budget(&self) -> Option<&ResourceBudget<R, Q>> {
        match &self.storage {
            DecodeStorage::Owned { input, .. } => input.as_ref(),
            DecodeStorage::Borrowed { input, .. } => match input {
                Some(input) => Some(&**input),
                None => None,
            },
        }
    }

    /// Returns the JSON value budget for read-only inspection.
    #[inline]
    pub const fn value_budget(&self) -> &JsonValueBudget<R, Q> {
        match &self.storage {
            DecodeStorage::Owned { value, .. } => value,
            DecodeStorage::Borrowed { value, .. } => value,
        }
    }

    /// Returns the JSON value budget for mutable traversal accounting.
    #[inline]
    pub fn value_budget_mut(&mut self) -> &mut JsonValueBudget<R, Q> {
        match &mut self.storage {
            DecodeStorage::Owned { value, .. } => value,
            DecodeStorage::Borrowed { value, .. } => value,
        }
    }
}

impl<R, Q> JsonDecodeSession<'static, R, Q>
where
    R: Clone,
    Q: ResourceQuantity,
{
    /// Creates an owned session from immutable limits.
    #[inline]
    pub fn owned(limits: JsonDecodeLimits<R, Q>) -> Self {
        let input = limits
            .input_bytes_limit()
            .cloned()
            .map(ResourceBudget::from_limit);
        let value = JsonValueBudget::new(limits.value_limits());
        Self {
            storage: DecodeStorage::Owned { input, value },
        }
    }
}

impl<R, Q> std::fmt::Debug for DecodeStorage<'_, R, Q>
where
    R: std::fmt::Debug,
    Q: ResourceQuantity,
{
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Owned { input, value } => formatter
                .debug_struct("Owned")
                .field("input", input)
                .field("value", value)
                .finish(),
            Self::Borrowed { input, value } => formatter
                .debug_struct("Borrowed")
                .field("input", &input.as_ref().map(|budget| budget.limit()))
                .field("value", value)
                .finish(),
        }
    }
}
