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

use super::super::output::JsonOutputAccounting;
use super::budgeted_display_collector::BudgetedDisplayCollector;
use super::display_budget_kind::DisplayBudgetKind;
use crate::decode::JsonSyntaxError;
use crate::lexical::JsonLexicalError;
use crate::lexical::JsonLexicalScanner;

/// Traversal context shared during JSON encoding budget checks.
pub(in crate::encode) struct JsonEncodeContext<'transaction, 'budget, R, Q>
where
    Q: ResourceQuantity,
{
    /// Caller-owned transaction charged by the traversal.
    pub(in crate::encode) transaction:
        &'transaction mut JsonValueTransaction<'budget, R, Q>,

    /// Live output accounting shared with the byte buffer.
    pub(in crate::encode) output:
        &'transaction RefCell<JsonOutputAccounting<'transaction, R, Q>>,
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

    /// Checks and charges a container before requesting its Serde compound.
    pub(super) fn enter_container<E>(
        &mut self,
        kind: JsonContainerKind,
        depth: usize,
    ) -> Result<(), E>
    where
        E: Error,
    {
        let result = self.transaction.try_enter_container(kind, depth);
        self.record(result)
    }

    /// Checks one prospective container count without charging value usage.
    pub(super) fn check_container_count<E>(
        &mut self,
        kind: JsonContainerKind,
        prospective: usize,
    ) -> Result<(), E>
    where
        E: Error,
    {
        let result = self.transaction.check_container_count(kind, prospective);
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
            let mut scanner =
                JsonLexicalScanner::at_depth(&mut *self.transaction, depth);
            scanner.scan(value.as_bytes())
        };
        match result {
            Ok(()) => Ok(()),
            Err(JsonLexicalError::Budget(error)) => self.record(Err(error)),
            Err(JsonLexicalError::Syntax(failure)) => {
                self.output.borrow_mut().record_syntax_error(
                    JsonSyntaxError::from_lexical(failure),
                );
                Err(E::custom("invalid raw JSON value"))
            }
        }
    }

    /// Formats one display value into a bounded collector.
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
}
