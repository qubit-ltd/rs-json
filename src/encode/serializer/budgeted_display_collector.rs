// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Bounded string sink for Serde `collect_str` hooks.

use std::cell::RefCell;
use std::fmt;

use qubit_budget::ResourceQuantity;

use super::display_budget_kind::DisplayBudgetKind;
use super::json_encode_context::JsonEncodeContext;

/// Collects display text without growing beyond the live output budget.
pub(super) struct BudgetedDisplayCollector<'context, 'transaction, 'budget, R, Q>
where
    Q: ResourceQuantity,
{
    /// Text accepted by the relevant budget so far.
    pub(super) text: String,

    /// Shared traversal state retaining typed budget errors.
    pub(super) context: &'context RefCell<JsonEncodeContext<'transaction, 'budget, R, Q>>,

    /// Value-budget meaning of the collected text.
    kind: DisplayBudgetKind,

    /// Root-inclusive position for a string measurement.
    depth: usize,
}

impl<'context, 'transaction, 'budget, R, Q>
    BudgetedDisplayCollector<'context, 'transaction, 'budget, R, Q>
where
    Q: ResourceQuantity,
{
    /// Creates an empty collector bound to the shared encode context.
    #[inline]
    pub(super) fn new(
        context: &'context RefCell<JsonEncodeContext<'transaction, 'budget, R, Q>>,
        kind: DisplayBudgetKind,
        depth: usize,
    ) -> Self {
        Self {
            text: String::new(),
            context,
            kind,
            depth,
        }
    }
}

impl<R, Q> fmt::Write for BudgetedDisplayCollector<'_, '_, '_, R, Q>
where
    R: Clone,
    Q: ResourceQuantity,
{
    /// Checks prospective output and value budgets before growing the string.
    fn write_str(&mut self, value: &str) -> fmt::Result {
        let next = self.text.len().checked_add(value.len()).ok_or(fmt::Error)?;
        let output_result = self.context.borrow().output.borrow().check_available(next);
        self.context
            .borrow_mut()
            .record::<fmt::Error>(output_result)?;
        self.context
            .borrow_mut()
            .check_display::<fmt::Error>(self.kind, self.depth, next)?;
        self.text.push_str(value);
        Ok(())
    }
}
