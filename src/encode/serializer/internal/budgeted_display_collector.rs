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

use super::DisplayBudgetKind;
use super::JsonEncodeContext;

/// Collects display text without growing beyond the live output budget.
///
/// # Type Parameters
///
/// * `R` - Resource identifier used by the shared encode budget.
/// * `Q` - Quantity representation used by the shared encode budget.
pub(in crate::encode) struct BudgetedDisplayCollector<'context, 'transaction, 'budget, R, Q>
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

impl<'context, 'transaction, 'budget, R, Q> BudgetedDisplayCollector<'context, 'transaction, 'budget, R, Q>
where
    Q: ResourceQuantity,
{
    /// Creates an empty collector bound to the shared encode context.
    ///
    /// # Parameters
    ///
    /// * `context` - Shared traversal and budget state used for each write.
    /// * `kind` - Value-budget category charged for the collected text.
    /// * `depth` - Root-inclusive depth used for string measurement.
    ///
    /// # Returns
    ///
    /// An empty collector that checks both output and value budgets before
    /// accepting text.
    #[must_use]
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
    ///
    /// # Parameters
    ///
    /// * `value` - Text to append after both budgets admit its full byte
    ///   length.
    ///
    /// # Returns
    ///
    /// `Ok(())` after appending all text, or `fmt::Error` when byte-length
    /// arithmetic overflows or either budget rejects the prospective total.
    ///
    /// # Errors
    ///
    /// Returns `fmt::Error` when byte-length arithmetic overflows or either
    /// the output budget or value budget rejects the prospective total.
    fn write_str(&mut self, value: &str) -> fmt::Result {
        let next = self.text.len().checked_add(value.len()).ok_or(fmt::Error)?;
        let output_result = self.context.borrow().output.borrow().check_available(next);
        self.context.borrow_mut().record::<fmt::Error>(output_result)?;
        self.context
            .borrow_mut()
            .check_display::<fmt::Error>(self.kind, self.depth, next)?;
        self.text.push_str(value);
        Ok(())
    }
}
