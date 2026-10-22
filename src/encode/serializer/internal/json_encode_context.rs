// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Shared state and bounded display collection for JSON encoding.

use std::cell::RefCell;
use std::fmt::Display;
use std::fmt::Write as _;

use qubit_budget::MeasuredBudgetError;
use qubit_budget::ResourceQuantity;
use qubit_budget::json::JsonContainerKind;
use qubit_budget::json::JsonMeasurement;
use qubit_budget::json::JsonValueTransaction;
use serde::ser::Error;

use super::super::super::output::JsonOutputAccounting;
use super::BudgetedDisplayCollector;
use super::DisplayBudgetKind;
use crate::decode::JsonSyntaxError;
use crate::encode::JsonSerializationError;
use crate::encode::JsonSerializationErrorKind;
use crate::lexical::JsonLexicalError;
use crate::lexical::JsonLexicalScanner;

/// Traversal context shared during JSON encoding budget checks.
///
/// The transaction supplies decoded-value measurements, while the shared
/// output accounting tracks the encoded bytes. The context keeps those two
/// budget dimensions coordinated during Serde serialization.
///
/// # Type Parameters
///
/// * `'transaction` - Borrow lifetime for the value transaction and shared
///   output accounting cell.
/// * `'budget` - Lifetime of the budget borrowed by the value transaction.
/// * `R` - Resource identifier used by measured budget errors.
/// * `Q` - Quantity type used by the value transaction and its measurements.
pub(in crate::encode) struct JsonEncodeContext<'transaction, 'budget, R, Q>
where
    Q: ResourceQuantity,
{
    /// Caller-owned transaction charged by the traversal.
    pub(in crate::encode) transaction: &'transaction mut JsonValueTransaction<'budget, R, Q>,

    /// Live output accounting shared with the byte buffer.
    pub(in crate::encode) output: &'transaction RefCell<JsonOutputAccounting<'transaction, R, Q>>,

    /// Whether any decoded-value resource dimension is configured.
    pub(in crate::encode) has_value_limits: bool,
}

impl<R, Q> JsonEncodeContext<'_, '_, R, Q>
where
    R: Clone,
    Q: ResourceQuantity,
{
    /// Records a structured serialization failure before returning the
    /// serializer-specific interruption error.
    ///
    /// # Type Parameters
    ///
    /// * `E` - Serde error type used to interrupt the current serialization.
    ///
    /// # Parameters
    ///
    /// * `self` - Shared budget and output state updated with the failure.
    /// * `kind` - Stable serialization failure category recorded for the outer
    ///   encoding operation.
    ///
    /// # Returns
    ///
    /// A serializer error constructed from the recorded failure.
    ///
    /// # Panics
    ///
    /// Panics if the shared output accounting cell is already borrowed.
    pub(super) fn serialization_error<E>(&mut self, kind: JsonSerializationErrorKind) -> E
    where
        E: Error,
    {
        let error = JsonSerializationError::new(kind);
        self.output.borrow_mut().record_serialization_error(error);
        E::custom(error)
    }

    /// Records one failed check before converting it into a Serde error.
    ///
    /// # Type Parameters
    ///
    /// * `E` - Serde error type used to report a failed budget check.
    ///
    /// # Parameters
    ///
    /// * `self` - Shared state used to retain measured violations.
    /// * `result` - Result of the budget operation to preserve or record.
    ///
    /// # Returns
    ///
    /// `Ok(())` when the check succeeded.
    ///
    /// # Errors
    ///
    /// Returns a custom Serde error after a measured violation has been
    /// recorded.
    ///
    /// # Panics
    ///
    /// Panics if the shared output accounting cell is already borrowed while
    /// a violation is recorded.
    pub(super) fn record<E>(&mut self, result: Result<(), MeasuredBudgetError<R, Q>>) -> Result<(), E>
    where
        E: Error,
    {
        result.map_err(|error| {
            self.output.borrow_mut().record_violation(error);
            E::custom("JSON resource budget exceeded")
        })
    }

    /// Stages one complete JSON measurement and maps any violation to Serde.
    ///
    /// # Type Parameters
    ///
    /// * `E` - Serde error type used to report a rejected measurement.
    ///
    /// # Parameters
    ///
    /// * `self` - Shared transaction and output state for this check.
    /// * `measurement` - Complete resource usage to admit to the value
    ///   transaction.
    ///
    /// # Returns
    ///
    /// `Ok(())` when value limits are disabled or the transaction accepts the
    ///   measurement.
    ///
    /// # Errors
    ///
    /// Returns a custom Serde error when the transaction rejects the
    /// measurement; the violation is recorded for the outer error.
    ///
    /// # Panics
    ///
    /// Panics if the shared output accounting cell is already borrowed while
    /// a violation is recorded.
    pub(super) fn admit<E>(&mut self, measurement: JsonMeasurement) -> Result<(), E>
    where
        E: Error,
    {
        if !self.has_value_limits {
            return Ok(());
        }
        let result = self.transaction.try_admit(measurement);
        self.record(result)
    }

    /// Checks prospective display text without charging the value transaction.
    ///
    /// The caller supplies the display text's byte length and its
    /// root-inclusive nesting depth. No value usage is committed by this
    /// check.
    ///
    /// # Type Parameters
    ///
    /// * `E` - Serde error type used to report a rejected display measurement.
    ///
    /// # Parameters
    ///
    /// * `self` - Shared transaction and output state for this check.
    /// * `kind` - Display resource dimension, such as a string or object key.
    /// * `depth` - Root-inclusive depth of the value in the encoded document.
    /// * `bytes` - Number of UTF-8 bytes in the prospective display text.
    ///
    /// # Returns
    ///
    /// `Ok(())` when limits are disabled or the prospective measurement fits.
    ///
    /// # Errors
    ///
    /// Returns a custom Serde error and records the violation when the
    /// prospective string or key exceeds its configured limits.
    ///
    /// # Panics
    ///
    /// Panics if the shared output accounting cell is already borrowed while
    /// a violation is recorded.
    pub(super) fn check_display<E>(&mut self, kind: DisplayBudgetKind, depth: usize, bytes: usize) -> Result<(), E>
    where
        E: Error,
    {
        if !self.has_value_limits {
            return Ok(());
        }
        let Some(measurement) = kind.measurement(depth, bytes) else {
            return Ok(());
        };
        let result = self.transaction.check_admit(measurement);
        self.record(result)
    }

    /// Checks and charges a container before requesting its Serde compound.
    ///
    /// # Type Parameters
    ///
    /// * `E` - Serde error type used to report a rejected container.
    ///
    /// # Parameters
    ///
    /// * `self` - Shared transaction and output state for this container.
    /// * `kind` - Sequence or map kind to enter.
    /// * `depth` - Root-inclusive depth of the container in the encoded value.
    ///
    /// # Returns
    ///
    /// `Ok(())` when limits are disabled or the container charge is accepted.
    ///
    /// # Errors
    ///
    /// Returns a custom Serde error and records the violation when the
    /// container exceeds its configured limits.
    ///
    /// # Panics
    ///
    /// Panics if the shared output accounting cell is already borrowed while
    /// a violation is recorded.
    pub(super) fn enter_container<E>(&mut self, kind: JsonContainerKind, depth: usize) -> Result<(), E>
    where
        E: Error,
    {
        if !self.has_value_limits {
            return Ok(());
        }
        let result = self.transaction.try_enter_container(kind, depth);
        self.record(result)
    }

    /// Checks one prospective container count without charging value usage.
    ///
    /// # Type Parameters
    ///
    /// * `E` - Serde error type used to report a rejected count.
    ///
    /// # Parameters
    ///
    /// * `self` - Shared transaction and output state for this count check.
    /// * `kind` - Sequence or map whose prospective length is checked.
    /// * `prospective` - Container length after the next item is added.
    ///
    /// # Returns
    ///
    /// `Ok(())` when limits are disabled or the prospective count fits.
    ///
    /// # Errors
    ///
    /// Returns a custom Serde error and records the violation when the count
    /// exceeds its configured limit.
    ///
    /// # Panics
    ///
    /// Panics if the shared output accounting cell is already borrowed while
    /// a violation is recorded.
    pub(super) fn check_container_count<E>(&mut self, kind: JsonContainerKind, prospective: usize) -> Result<(), E>
    where
        E: Error,
    {
        if !self.has_value_limits {
            return Ok(());
        }
        let result = self.transaction.check_container_count(kind, prospective);
        self.record(result)
    }

    /// Checks and charges one raw JSON fragment before it reaches the writer.
    ///
    /// The fragment length is a safe lower bound for the complete output size.
    /// Structural traversal starts at `depth`, the fragment's root-inclusive
    /// position in the final document.
    ///
    /// # Type Parameters
    ///
    /// * `E` - Serde error type used to report budget or syntax failures.
    ///
    /// # Parameters
    ///
    /// * `self` - Shared transaction and output state used for preflight.
    /// * `value` - Complete raw JSON fragment to check before writing.
    /// * `depth` - Root-inclusive depth of the fragment in the final document.
    ///
    /// # Returns
    ///
    /// `Ok(())` after the output capacity and fragment structure pass their
    /// checks.
    ///
    /// # Errors
    ///
    /// Returns a custom Serde error for an output/value budget violation or
    /// invalid JSON syntax; the corresponding failure is recorded for the
    /// outer encoding error.
    ///
    /// # Panics
    ///
    /// Panics if the shared output accounting cell is already borrowed while
    /// checking capacity or recording a failure.
    pub(super) fn preflight_raw<E>(&mut self, value: &str, depth: usize) -> Result<(), E>
    where
        E: Error,
        R: Clone,
    {
        let output = self.output.borrow().check_available(value.len());
        self.record(output)?;
        let result = {
            let mut scanner = JsonLexicalScanner::at_depth(&mut *self.transaction, depth, self.has_value_limits);
            scanner.scan(value.as_bytes())
        };
        match result {
            Ok(()) => Ok(()),
            Err(JsonLexicalError::Budget(error)) => self.record(Err(error)),
            Err(JsonLexicalError::Syntax(failure)) => {
                self.output
                    .borrow_mut()
                    .record_syntax_error(JsonSyntaxError::from_lexical(failure));
                Err(E::custom("invalid raw JSON value"))
            }
        }
    }

    /// Formats one display value into a bounded collector.
    ///
    /// The collector checks output capacity as text is written, then admits the
    /// completed string's value measurement when that resource dimension is
    /// configured.
    ///
    /// # Type Parameters
    ///
    /// * `E` - Serde error type used for formatting and budget failures.
    /// * `T` - Display value formatted into the bounded collector.
    ///
    /// # Parameters
    ///
    /// * `context` - Shared encoding state used for output and value checks.
    /// * `value` - Display value to format.
    /// * `kind` - String or key resource dimension applied to the text.
    /// * `depth` - Root-inclusive depth of the value in the encoded document.
    ///
    /// # Returns
    ///
    /// The rendered string after formatting and all configured budget checks
    /// succeed.
    ///
    /// # Errors
    ///
    /// Returns a custom Serde error when formatting fails or either output or
    /// value accounting rejects the text.
    ///
    /// # Panics
    ///
    /// Panics if the context RefCell is already borrowed when the collector or
    /// final value admission needs mutable access.
    pub(super) fn collect_display<E, T>(
        context: &RefCell<Self>,
        value: &T,
        kind: DisplayBudgetKind,
        depth: usize,
    ) -> Result<String, E>
    where
        E: Error,
        T: Display + ?Sized,
    {
        let mut collector = BudgetedDisplayCollector::new(context, kind, depth);
        write!(&mut collector, "{value}").map_err(|_| {
            context
                .borrow_mut()
                .serialization_error(JsonSerializationErrorKind::DisplayFormattingFailed)
        })?;
        let text = collector.text;
        if let Some(measurement) = kind.measurement(depth, text.len()) {
            collector.context.borrow_mut().admit(measurement)?;
        }
        Ok(text)
    }
}
