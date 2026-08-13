// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Bounded output buffer for budget-aware JSON encoding.
// qubit-style: allow source-test-pair
// qubit-style: allow multiple-public-types

use std::cell::RefCell;
use std::io;
use std::io::Write;
use std::rc::Rc;

use qubit_budget::MeasuredBudgetError;
use qubit_budget::ResourceBudget;
use qubit_budget::ResourceQuantity;

use super::super::JsonSerdeError;

/// Shared transactional output accounting used by the serializer and buffer.
pub(in crate::budget) struct JsonOutputAccounting<'a, R, Q>
where
    Q: ResourceQuantity,
{
    /// Optional operation-local output budget charged as bytes are appended.
    output: Option<&'a mut ResourceBudget<R, Q>>,

    /// First budget violation hidden behind a Serde or I/O error.
    violation: Option<MeasuredBudgetError<R, Q>>,
}

impl<'a, R, Q> JsonOutputAccounting<'a, R, Q>
where
    Q: ResourceQuantity,
{
    /// Creates live accounting over the caller-owned output budget.
    ///
    /// # Parameters
    ///
    /// * `output` - Optional output-byte budget for the encode session.
    ///
    /// # Returns
    ///
    /// Fresh accounting with no recorded violation.
    #[inline]
    pub(in crate::budget) const fn new(output: Option<&'a mut ResourceBudget<R, Q>>) -> Self {
        Self {
            output,
            violation: None,
        }
    }

    /// Checks an output lower bound against current remaining capacity.
    ///
    /// # Parameters
    ///
    /// * `amount` - Complete prospective byte count.
    ///
    /// # Returns
    ///
    /// `Ok(())` when the lower bound fits or output accounting is disabled.
    ///
    /// # Errors
    ///
    /// Returns the output budget error without consuming any capacity when the
    /// amount exceeds the live remaining capacity.
    #[inline]
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

    /// Consumes emitted output bytes against current remaining capacity.
    ///
    /// # Parameters
    ///
    /// * `amount` - Number of bytes accepted by the output buffer.
    ///
    /// # Returns
    ///
    /// `Ok(())` after consuming the bytes or when accounting is disabled.
    ///
    /// # Errors
    ///
    /// Returns the output budget error while leaving capacity unchanged when
    /// the amount exceeds the live remaining capacity.
    #[inline]
    fn consume(&mut self, amount: usize) -> Result<(), MeasuredBudgetError<R, Q>>
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

    /// Records a budget violation only when no earlier violation exists.
    ///
    /// # Parameters
    ///
    /// * `error` - Typed budget error hidden by Serde or I/O.
    pub(super) fn record_violation(&mut self, error: MeasuredBudgetError<R, Q>) {
        if self.violation.is_none() {
            self.violation = Some(error);
        }
    }

    /// Takes the first recorded budget violation, if one exists.
    ///
    /// # Returns
    ///
    /// The first typed violation, or `None` when no budget check failed.
    fn take_violation(&mut self) -> Option<MeasuredBudgetError<R, Q>> {
        self.violation.take()
    }
}

/// Accumulates JSON bytes only while the configured output budget permits it.
pub(in crate::budget) struct JsonOutputBuffer<'a, R, Q>
where
    Q: ResourceQuantity,
{
    /// Bytes accepted by the output budget so far.
    bytes: Vec<u8>,

    /// Accounting shared with the online serializer checks.
    accounting: Rc<RefCell<JsonOutputAccounting<'a, R, Q>>>,
}

impl<'a, R, Q> JsonOutputBuffer<'a, R, Q>
where
    Q: ResourceQuantity,
{
    /// Creates an empty bounded output buffer.
    ///
    /// # Parameters
    ///
    /// * `accounting` - Shared live budget and first-violation state.
    ///
    /// # Returns
    ///
    /// An empty writer with no recorded violation.
    #[inline]
    pub(in crate::budget) const fn new(
        accounting: Rc<RefCell<JsonOutputAccounting<'a, R, Q>>>,
    ) -> Self {
        Self {
            bytes: Vec::new(),
            accounting,
        }
    }

    /// Resolves serialization and returns the accepted bytes or original error.
    ///
    /// # Parameters
    ///
    /// * `result` - Result returned by the JSON serializer.
    /// # Returns
    ///
    /// The complete bounded output when serialization succeeds.
    ///
    /// # Errors
    ///
    /// Returns [`JsonSerdeError::Budget`] for a recorded output violation,
    /// taking precedence over its erased I/O representation. Otherwise returns
    /// [`JsonSerdeError::Json`] for the serializer failure.
    pub(in crate::budget) fn into_result(
        self,
        result: Result<(), serde_json::Error>,
    ) -> Result<Vec<u8>, JsonSerdeError<R, Q>> {
        let violation = self.accounting.borrow_mut().take_violation();
        if let Some(error) = violation {
            return Err(JsonSerdeError::from(error));
        }
        result.map_err(JsonSerdeError::Json)?;
        Ok(self.bytes)
    }
}

impl<R, Q> Write for JsonOutputBuffer<'_, R, Q>
where
    R: Clone,
    Q: ResourceQuantity,
{
    /// Appends one complete input slice after checking the resulting length.
    ///
    /// The buffer remains unchanged if arithmetic overflows or the output-byte
    /// limit is exceeded.
    fn write(&mut self, input: &[u8]) -> io::Result<usize> {
        let next = self
            .bytes
            .len()
            .checked_add(input.len())
            .ok_or_else(|| io::Error::other("JSON output length overflow"))?;
        let amount = next - self.bytes.len();
        let mut accounting = self.accounting.borrow_mut();
        if let Err(error) = accounting.consume(amount) {
            accounting.record_violation(error);
            return Err(io::Error::other("JSON output budget exceeded"));
        }
        drop(accounting);
        self.bytes.extend_from_slice(input);
        Ok(input.len())
    }

    /// Flushes the in-memory buffer without performing external I/O.
    #[inline(always)]
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
