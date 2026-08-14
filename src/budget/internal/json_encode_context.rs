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
use serde::ser::Error;

use super::super::JsonSerdeError;
use super::super::JsonValueBudget;
use super::JsonLexicalPreflight;
use super::display_budget_kind::DisplayBudgetKind;
use super::json_output_buffer::JsonOutputAccounting;

pub(super) struct JsonEncodeContext<'a, R, Q>
where
    Q: ResourceQuantity,
{
    /// Caller-owned budget charged by the traversal.
    pub(super) budget: &'a mut JsonValueBudget<R, Q>,

    /// Live output accounting shared with the byte buffer.
    pub(super) output: Rc<RefCell<JsonOutputAccounting<'a, R, Q>>>,
}

impl<R, Q> JsonEncodeContext<'_, R, Q>
where
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
        match JsonLexicalPreflight::at_depth(self.budget, depth)
            .inspect(value.as_bytes())
        {
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
struct BudgetedDisplayCollector<'a, R, Q>
where
    Q: ResourceQuantity,
{
    /// Text accepted by the relevant budget so far.
    text: String,

    /// Shared traversal state retaining typed budget errors.
    context: Rc<RefCell<JsonEncodeContext<'a, R, Q>>>,

    /// Resource semantics applied to the collected text.
    kind: DisplayBudgetKind,
}

impl<'a, R, Q> BudgetedDisplayCollector<'a, R, Q>
where
    Q: ResourceQuantity,
{
    /// Creates an empty collector for one resource kind.
    fn new(
        context: Rc<RefCell<JsonEncodeContext<'a, R, Q>>>,
        kind: DisplayBudgetKind,
    ) -> Self {
        Self {
            text: String::new(),
            context,
            kind,
        }
    }
}

impl<R, Q> fmt::Write for BudgetedDisplayCollector<'_, R, Q>
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
        let point_result = {
            let context = self.context.borrow();
            match self.kind {
                DisplayBudgetKind::String => {
                    context.budget.check_string_bytes_usize(next)
                }
                DisplayBudgetKind::Key => {
                    context.budget.check_key_bytes_usize(next)
                }
                DisplayBudgetKind::Number => {
                    context.budget.check_number_bytes_usize(next)
                }
                DisplayBudgetKind::RawOutput => Ok(()),
            }
        };
        self.context
            .borrow_mut()
            .record::<fmt::Error>(point_result)?;
        self.text.push_str(value);
        Ok(())
    }
}

/// Formats one display value into a budgeted collector.
pub(super) fn collect_display<'a, E, T, R, Q>(
    value: &T,
    context: Rc<RefCell<JsonEncodeContext<'a, R, Q>>>,
    kind: DisplayBudgetKind,
) -> Result<String, E>
where
    E: Error,
    T: Display + ?Sized,
    R: Clone,
    Q: ResourceQuantity,
{
    let mut collector = BudgetedDisplayCollector::new(context, kind);
    write!(&mut collector, "{value}")
        .map_err(|_| E::custom("JSON display text budget exceeded"))?;
    let text = collector.text;
    let payload_result = {
        let mut context = collector.context.borrow_mut();
        match kind {
            DisplayBudgetKind::String => {
                context.budget.consume_string_bytes_usize(text.len())
            }
            DisplayBudgetKind::Key => {
                context.budget.consume_key_bytes_usize(text.len())
            }
            DisplayBudgetKind::Number => {
                context.budget.consume_number_bytes_usize(text.len())
            }
            DisplayBudgetKind::RawOutput => Ok(()),
        }
    };
    collector.context.borrow_mut().record::<E>(payload_result)?;
    Ok(text)
}
