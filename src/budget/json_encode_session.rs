// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tracks mutable accounting for one JSON encoding operation.

use qubit_budget::BudgetError;
use qubit_budget::MeasuredBudgetError;
use qubit_budget::ResourceBudget;
use qubit_budget::ResourceQuantity;

use super::JsonEncodeLimits;
use super::JsonResource;
use super::JsonValueBudget;

#[derive(Debug, PartialEq, Eq)]
enum JsonEncodeStorage<'a, R, Q>
where
    Q: ResourceQuantity,
{
    Owned {
        output: Option<ResourceBudget<R, Q>>,
        value: JsonValueBudget<R, Q>,
    },
    Borrowed {
        output: Option<&'a mut ResourceBudget<R, Q>>,
        value: &'a mut JsonValueBudget<R, Q>,
    },
}

/// Mutable state for one JSON encoding operation.
///
/// Output bytes and JSON value resources are intentionally separate: only
/// output bytes are directional, while the embedded [`JsonValueBudget`]
/// accounts for encoded JSON values.
#[must_use]
#[derive(Debug, PartialEq, Eq)]
pub struct JsonEncodeSession<'a, R = JsonResource, Q = usize>
where
    Q: ResourceQuantity,
{
    storage: JsonEncodeStorage<'a, R, Q>,
}

impl<R, Q> JsonEncodeSession<'static, R, Q>
where
    R: Clone,
    Q: ResourceQuantity,
{
    /// Creates fresh mutable accounting for one JSON encoding operation.
    #[inline]
    pub fn owned(limits: JsonEncodeLimits<R, Q>) -> Self {
        let output = limits
            .output_bytes_limit()
            .cloned()
            .map(ResourceBudget::from_limit);
        let value = JsonValueBudget::new(limits.value_limits());
        Self {
            storage: JsonEncodeStorage::Owned { output, value },
        }
    }
}

impl<'a, R, Q> JsonEncodeSession<'a, R, Q>
where
    R: Clone,
    Q: ResourceQuantity,
{
    /// Creates mutable accounting over caller-owned output and value budgets.
    #[inline]
    pub fn borrowing(
        output: Option<&'a mut ResourceBudget<R, Q>>,
        value: &'a mut JsonValueBudget<R, Q>,
    ) -> Self {
        Self {
            storage: JsonEncodeStorage::Borrowed { output, value },
        }
    }

    /// Consumes output bytes atomically for this encoding operation.
    ///
    /// A failed request leaves the remaining output capacity unchanged.
    #[inline]
    pub fn consume_output_bytes(&mut self, amount: Q) -> Result<(), BudgetError<R, Q>> {
        match &mut self.storage {
            JsonEncodeStorage::Owned { output, .. } => match output {
                Some(output) => output.try_consume(amount),
                None => Ok(()),
            },
            JsonEncodeStorage::Borrowed { output, .. } => match output {
                Some(output) => output.try_consume(amount),
                None => Ok(()),
            },
        }
    }

    /// Converts and consumes native output bytes atomically.
    #[inline]
    pub fn consume_output_bytes_usize(
        &mut self,
        amount: usize,
    ) -> Result<(), MeasuredBudgetError<R, Q>> {
        let output = match &mut self.storage {
            JsonEncodeStorage::Owned { output, .. } => output.as_mut(),
            JsonEncodeStorage::Borrowed { output, .. } => output.as_deref_mut(),
        };
        let Some(output) = output else {
            return Ok(());
        };
        let amount = Q::try_from_usize(amount)
            .map_err(|source| MeasuredBudgetError::quantity(output.resource().clone(), source))?;
        output
            .try_consume(amount)
            .map_err(MeasuredBudgetError::from)
    }

    /// Returns the configured cumulative output-byte maximum.
    #[must_use]
    #[inline(always)]
    pub const fn max_output_bytes(&self) -> Option<Q> {
        match &self.storage {
            JsonEncodeStorage::Owned { output, .. } => match output {
                Some(output) => Some(output.limit()),
                None => None,
            },
            JsonEncodeStorage::Borrowed { output, .. } => match output {
                Some(output) => Some(output.limit()),
                None => None,
            },
        }
    }

    /// Returns the output-byte budget, when output accounting is configured.
    #[must_use = "the output budget tracks bytes emitted by this encode operation"]
    #[inline(always)]
    pub const fn output_budget(&self) -> Option<&ResourceBudget<R, Q>> {
        match &self.storage {
            JsonEncodeStorage::Owned { output, .. } => match output {
                Some(output) => Some(output),
                None => None,
            },
            JsonEncodeStorage::Borrowed { output, .. } => match output {
                Some(output) => Some(&**output),
                None => None,
            },
        }
    }

    /// Returns the JSON value budget for read-only inspection.
    #[must_use = "the value budget tracks encoded JSON nodes, structure and payload"]
    #[inline(always)]
    pub const fn value_budget(&self) -> &JsonValueBudget<R, Q> {
        match &self.storage {
            JsonEncodeStorage::Owned { value, .. } => value,
            JsonEncodeStorage::Borrowed { value, .. } => value,
        }
    }

    /// Returns the JSON value budget for mutable JSON traversal accounting.
    #[inline(always)]
    pub fn value_budget_mut(&mut self) -> &mut JsonValueBudget<R, Q> {
        match &mut self.storage {
            JsonEncodeStorage::Owned { value, .. } => value,
            JsonEncodeStorage::Borrowed { value, .. } => value,
        }
    }

    /// Splits mutable output and value accounting for one online traversal.
    ///
    /// This crate-private operation lets the Serde adapter enforce value
    /// limits before delegation while the output writer independently charges
    /// bytes as they are emitted.
    #[inline(always)]
    pub(crate) fn split_mut(
        &mut self,
    ) -> (
        Option<&mut ResourceBudget<R, Q>>,
        &mut JsonValueBudget<R, Q>,
    ) {
        match &mut self.storage {
            JsonEncodeStorage::Owned { output, value } => (output.as_mut(), value),
            JsonEncodeStorage::Borrowed { output, value } => (output.as_deref_mut(), value),
        }
    }
}
