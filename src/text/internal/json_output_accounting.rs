// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Shared live output accounting for strict JSON encoding.

use qubit_budget::MeasuredBudgetError;
use qubit_budget::ResourceBudget;
use qubit_budget::ResourceQuantity;

use crate::text::JsonSyntaxError;

/// Tracks the output budget and the first erased budget violation.
pub(in crate::text) struct JsonOutputAccounting<'a, R, Q>
where
    Q: ResourceQuantity,
{
    /// Optional operation-local output budget.
    output: Option<&'a mut ResourceBudget<R, Q>>,

    /// First budget violation hidden behind a Serde or I/O error.
    violation: Option<MeasuredBudgetError<R, Q>>,

    /// First raw JSON syntax failure hidden behind a Serde error.
    syntax_error: Option<JsonSyntaxError>,
}

impl<'a, R, Q> JsonOutputAccounting<'a, R, Q>
where
    Q: ResourceQuantity,
{
    /// Creates live accounting over the caller-owned output budget.
    #[inline]
    pub(in crate::text) const fn new(output: Option<&'a mut ResourceBudget<R, Q>>) -> Self {
        Self {
            output,
            violation: None,
            syntax_error: None,
        }
    }

    /// Checks an output lower bound without consuming capacity.
    pub(super) fn check_available(&self, amount: usize) -> Result<(), MeasuredBudgetError<R, Q>>
    where
        R: Clone,
    {
        self.output.as_deref().map_or(Ok(()), |output| {
            let amount = Q::try_from_usize(amount).map_err(|source| {
                MeasuredBudgetError::quantity(output.resource().clone(), source)
            })?;
            output
                .check_available(amount)
                .map_err(MeasuredBudgetError::from)
        })
    }

    /// Consumes bytes accepted by the output destination.
    pub(super) fn consume(&mut self, amount: usize) -> Result<(), MeasuredBudgetError<R, Q>>
    where
        R: Clone,
    {
        self.output.as_deref_mut().map_or(Ok(()), |output| {
            let amount = Q::try_from_usize(amount).map_err(|source| {
                MeasuredBudgetError::quantity(output.resource().clone(), source)
            })?;
            output
                .try_consume(amount)
                .map_err(MeasuredBudgetError::from)
        })
    }

    /// Records the first budget violation hidden by a writer error.
    pub(super) fn record_violation(&mut self, error: MeasuredBudgetError<R, Q>) {
        if self.violation.is_none() {
            self.violation = Some(error);
        }
    }

    /// Takes the first recorded violation, if one exists.
    pub(super) fn take_violation(&mut self) -> Option<MeasuredBudgetError<R, Q>> {
        self.violation.take()
    }

    /// Records the first raw JSON syntax failure.
    pub(super) fn record_syntax_error(&mut self, error: JsonSyntaxError) {
        if self.syntax_error.is_none() {
            self.syntax_error = Some(error);
        }
    }

    /// Takes the first recorded raw JSON syntax failure.
    pub(super) fn take_syntax_error(&mut self) -> Option<JsonSyntaxError> {
        self.syntax_error.take()
    }
}
