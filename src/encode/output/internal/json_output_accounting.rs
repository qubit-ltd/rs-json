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

use crate::decode::JsonSyntaxError;
use crate::encode::JsonSerializationError;
use crate::encode::JsonSerializationErrorKind;

/// Tracks the output budget and the first erased budget violation.
///
/// # Type Parameters
///
/// * `R` - Resource identity tracked by the output budget.
/// * `Q` - Quantity representation used by the output budget.
///
/// # Lifetime Parameters
///
/// * `'a` - Borrow lifetime of the operation-local output budget.
pub(in crate::encode) struct JsonOutputAccounting<'a, R, Q>
where
    Q: ResourceQuantity,
{
    /// Optional operation-local output budget.
    output: Option<&'a mut ResourceBudget<R, Q>>,

    /// First budget violation hidden behind a Serde or I/O error.
    violation: Option<MeasuredBudgetError<R, Q>>,

    /// First raw JSON syntax failure hidden behind a Serde error.
    syntax_error: Option<JsonSyntaxError>,

    /// First structured serialization failure hidden behind a Serde error.
    serialization_error: Option<JsonSerializationError>,
}

impl<'a, R, Q> JsonOutputAccounting<'a, R, Q>
where
    Q: ResourceQuantity,
{
    /// Creates live accounting over the caller-owned output budget.
    ///
    /// # Parameters
    ///
    /// * `output` - Optional mutable budget whose capacity this accounting
    ///   tracks.
    ///
    /// # Returns
    ///
    /// Accounting initialized with no recorded failures.
    #[inline]
    pub(in crate::encode) const fn new(output: Option<&'a mut ResourceBudget<R, Q>>) -> Self {
        Self {
            output,
            violation: None,
            syntax_error: None,
            serialization_error: None,
        }
    }

    /// Reports whether this operation enforces an output-byte limit.
    ///
    /// # Returns
    ///
    /// `true` when an output budget is attached, or `false` for unbounded
    /// output.
    #[must_use]
    #[inline(always)]
    pub(in crate::encode) const fn has_output_budget(&self) -> bool {
        self.output.is_some()
    }

    /// Returns the operation-local output capacity still available.
    ///
    /// `None` means output bytes are not bounded for this operation.
    ///
    /// # Returns
    ///
    /// `Some(remaining)` for a bounded operation, or `None` when no output
    /// budget is attached.
    #[must_use]
    #[inline(always)]
    pub(in crate::encode) fn remaining(&self) -> Option<Q> {
        self.output.as_deref().map(ResourceBudget::remaining)
    }

    /// Checks an output lower bound without consuming capacity.
    ///
    /// # Parameters
    ///
    /// * `amount` - Number of output bytes to check.
    ///
    /// # Returns
    ///
    /// `Ok(())` when the amount fits or output is unbounded.
    ///
    /// # Errors
    ///
    /// Returns a measured budget error when the amount cannot be represented
    /// or exceeds the available capacity.
    pub(in crate::encode) fn check_available(&self, amount: usize) -> Result<(), MeasuredBudgetError<R, Q>>
    where
        R: Clone,
    {
        self.output.as_deref().map_or(Ok(()), |output| {
            let amount = Q::try_from_usize(amount)
                .map_err(|source| MeasuredBudgetError::quantity(output.resource().clone(), source))?;
            output.check_available(amount).map_err(MeasuredBudgetError::from)
        })
    }

    /// Consumes bytes accepted by the output destination.
    ///
    /// # Parameters
    ///
    /// * `amount` - Number of bytes accepted by the output destination.
    ///
    /// # Returns
    ///
    /// `Ok(())` when the amount is charged or output is unbounded.
    ///
    /// # Errors
    ///
    /// Returns a measured budget error when the amount cannot be represented
    /// or the budget rejects consumption.
    pub(in crate::encode) fn consume(&mut self, amount: usize) -> Result<(), MeasuredBudgetError<R, Q>>
    where
        R: Clone,
    {
        self.output.as_deref_mut().map_or(Ok(()), |output| {
            let amount = Q::try_from_usize(amount)
                .map_err(|source| MeasuredBudgetError::quantity(output.resource().clone(), source))?;
            output.try_consume(amount).map_err(MeasuredBudgetError::from)
        })
    }

    /// Records the first budget violation hidden by a writer error.
    ///
    /// # Parameters
    ///
    /// * `error` - Budget violation to preserve unless an earlier one exists.
    pub(in crate::encode) fn record_violation(&mut self, error: MeasuredBudgetError<R, Q>) {
        if self.violation.is_none() {
            self.violation = Some(error);
        }
    }

    /// Takes the first recorded violation, if one exists.
    ///
    /// # Returns
    ///
    /// The recorded violation, or `None` when no violation was recorded.
    #[must_use]
    #[inline(always)]
    pub(in crate::encode) fn take_violation(&mut self) -> Option<MeasuredBudgetError<R, Q>> {
        self.violation.take()
    }

    /// Records the first raw JSON syntax failure.
    ///
    /// # Parameters
    ///
    /// * `error` - Syntax failure to preserve unless an earlier one exists.
    pub(in crate::encode) fn record_syntax_error(&mut self, error: JsonSyntaxError) {
        if self.syntax_error.is_none() {
            self.syntax_error = Some(error);
        }
    }

    /// Takes the first recorded raw JSON syntax failure.
    ///
    /// # Returns
    ///
    /// The recorded syntax failure, or `None` when none was recorded.
    #[must_use]
    #[inline(always)]
    pub(in crate::encode) fn take_syntax_error(&mut self) -> Option<JsonSyntaxError> {
        self.syntax_error.take()
    }

    /// Records the first structured serialization failure.
    ///
    /// # Parameters
    ///
    /// * `error` - Serialization failure to preserve unless an earlier one
    ///   exists.
    pub(in crate::encode) fn record_serialization_error(&mut self, error: JsonSerializationError) {
        if self.serialization_error.is_none() {
            self.serialization_error = Some(error);
        }
    }

    /// Records an opaque nested serialization failure without retaining its
    /// text.
    ///
    /// Earlier structured serialization failures remain authoritative.
    #[inline]
    pub(in crate::encode) fn record_custom_serialization_error(&mut self) {
        self.record_serialization_error(JsonSerializationError::new(
            JsonSerializationErrorKind::CustomSerialization,
        ));
    }

    /// Takes the first structured serialization failure, if one exists.
    ///
    /// # Returns
    ///
    /// The recorded serialization failure, or `None` when none was recorded.
    #[must_use]
    #[inline(always)]
    pub(in crate::encode) fn take_serialization_error(&mut self) -> Option<JsonSerializationError> {
        self.serialization_error.take()
    }

    /// Takes the recorded serialization failure or returns the opaque fallback
    /// used for arbitrary third-party Serde errors.
    ///
    /// # Returns
    ///
    /// The recorded serialization failure, or a custom-serialization fallback.
    pub(in crate::encode) fn take_serialization_error_or_custom(&mut self) -> JsonSerializationError {
        self.take_serialization_error()
            .unwrap_or_else(|| JsonSerializationError::new(JsonSerializationErrorKind::CustomSerialization))
    }
}
