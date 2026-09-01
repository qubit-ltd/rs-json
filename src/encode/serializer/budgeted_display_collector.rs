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
    /// Checks cumulative length before growing the string.
    fn write_str(&mut self, value: &str) -> fmt::Result {
        let next = self.text.len().checked_add(value.len()).ok_or(fmt::Error)?;
        let output_result = self.context.borrow().output.borrow().check_available(next);
        self.context
            .borrow_mut()
            .record::<fmt::Error>(output_result)?;
        self.text.push_str(value);
        Ok(())
    }
}
