// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Shared state and bounded display collection for JSON encoding.

use std::cell::RefCell;
use std::fmt;
use std::fmt::Display;
use std::fmt::Write as _;
use std::rc::Rc;

use qubit_budget::MeasuredBudgetError;
use qubit_budget::ResourceQuantity;
use qubit_budget::json::JsonMeasurement;
use qubit_budget::json::JsonValueTransaction;
use serde::ser::Error;

use super::super::JsonSerdeError;
use super::JsonLexicalPreflight;
use super::display_budget_kind::DisplayBudgetKind;
use super::json_output_buffer::JsonOutputAccounting;

pub(in crate::budget) struct JsonEncodeContext<'transaction, 'budget, R, Q>
where
    Q: ResourceQuantity,
{
    /// Caller-owned transaction charged by the traversal.
    pub(in crate::budget) transaction:
        &'transaction mut JsonValueTransaction<'budget, R, Q>,

    /// Live output accounting shared with the byte buffer.
    pub(in crate::budget) output:
        Rc<RefCell<JsonOutputAccounting<'transaction, R, Q>>>,
}

impl<R, Q> JsonEncodeContext<'_, '_, R, Q>
where
    R: Clone,
    Q: ResourceQuantity,
{
    /// Records one failed check before converting it into a Serde error.
    pub(super) fn record<E>(
        &mut self,
        result: Result<(), MeasuredBudgetError<R, Q>>,
    ) -> Result<(), E>
    where
        E: Error,
    {
        result.map_err(|error| {
            self.output.borrow_mut().record_violation(error);
            E::custom("JSON resource budget exceeded")
        })
    }

    /// Stages one complete JSON measurement and maps any violation to Serde.
    pub(super) fn admit<E>(
        &mut self,
        measurement: JsonMeasurement,
    ) -> Result<(), E>
    where
        E: Error,
    {
        let result = self.transaction.try_admit(measurement);
        self.record(result)
    }

    /// Checks and charges one raw JSON fragment before it reaches the writer.
    ///
    /// The fragment length is a safe lower bound for the complete output size.
    /// Structural traversal starts at `depth`, the fragment's root-inclusive
    /// position in the final document.
    pub(super) fn preflight_raw<E>(
        &mut self,
        value: &str,
        depth: usize,
    ) -> Result<(), E>
    where
        E: Error,
        R: Clone,
    {
        let output = self.output.borrow().check_available(value.len());
        self.record(output)?;
        let result = {
            let mut preflight =
                JsonLexicalPreflight::at_depth(&mut *self.transaction, depth);
            preflight.inspect(value.as_bytes())
        };
        match result {
            Ok(()) => Ok(()),
            Err(JsonSerdeError::Budget(error)) => {
                self.record(Err(error.into()))
            }
            Err(JsonSerdeError::Quantity { resource, source }) => {
                self.record(Err(MeasuredBudgetError::Quantity {
                    resource,
                    source,
                }))
            }
            Err(JsonSerdeError::Json(_) | JsonSerdeError::Io(_)) => {
                Err(E::custom("invalid raw JSON value"))
            }
            Err(JsonSerdeError::Syntax(_)) => {
                Err(E::custom("invalid raw JSON value"))
            }
        }
    }
}

/// Bounded string sink used by Serde `collect_str` hooks.
///
/// A fallible `Display` wrapper cannot safely be delegated to serde_json:
/// its streaming string adapter assumes every formatting error came from its
/// writer, while private Number/RawValue emitters may call `to_string` first.
/// This collector therefore owns the failure boundary and caps allocation
/// before passing an already bounded `str` to the inner serializer.
struct BudgetedDisplayCollector<'context, 'transaction, 'budget, R, Q>
where
    Q: ResourceQuantity,
{
    /// Text accepted by the relevant budget so far.
    text: String,

    /// Shared traversal state retaining typed budget errors.
    context: &'context RefCell<JsonEncodeContext<'transaction, 'budget, R, Q>>,
}

impl<'context, 'transaction, 'budget, R, Q>
    BudgetedDisplayCollector<'context, 'transaction, 'budget, R, Q>
where
    Q: ResourceQuantity,
{
    /// Creates an empty collector bound to the shared encode context.
    fn new(
        context: &'context RefCell<
            JsonEncodeContext<'transaction, 'budget, R, Q>,
        >,
    ) -> Self {
        Self {
            text: String::new(),
            context,
        }
    }
}

impl<R, Q> fmt::Write for BudgetedDisplayCollector<'_, '_, '_, R, Q>
where
    R: Clone,
    Q: ResourceQuantity,
{
    /// Checks the cumulative formatted length before growing the string.
    fn write_str(&mut self, value: &str) -> fmt::Result {
        let next =
            self.text.len().checked_add(value.len()).ok_or(fmt::Error)?;
        let output_result =
            self.context.borrow().output.borrow().check_available(next);
        self.context
            .borrow_mut()
            .record::<fmt::Error>(output_result)?;
        self.text.push_str(value);
        Ok(())
    }
}

/// Formats one display value into a budgeted collector.
pub(super) fn collect_display<'context, 'transaction, 'budget, E, T, R, Q>(
    value: &T,
    context: &'context RefCell<JsonEncodeContext<'transaction, 'budget, R, Q>>,
    kind: DisplayBudgetKind,
    depth: usize,
) -> Result<String, E>
where
    E: Error,
    T: Display + ?Sized,
    R: Clone,
    Q: ResourceQuantity,
{
    let mut collector = BudgetedDisplayCollector::new(context);
    write!(&mut collector, "{value}")
        .map_err(|_| E::custom("JSON display text budget exceeded"))?;
    let text = collector.text;
    {
        let mut context = collector.context.borrow_mut();
        match kind {
            DisplayBudgetKind::String => {
                context.admit(JsonMeasurement::String {
                    depth,
                    bytes: text.len(),
                })
            }
            DisplayBudgetKind::Key => {
                context.admit(JsonMeasurement::Key { bytes: text.len() })
            }
            DisplayBudgetKind::Number => {
                context.admit(JsonMeasurement::Number {
                    depth,
                    bytes: text.len(),
                })
            }
            DisplayBudgetKind::RawOutput => Ok(()),
        }?;
    }
    Ok(text)
}
