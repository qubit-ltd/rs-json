// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Provides reusable full-tree JSON budget accounting.

use qubit_budget::MeasuredBudgetError;
use qubit_budget::ResourceQuantity;
use qubit_budget::json::JsonResource;
use qubit_budget::json::JsonValueBudget;
use qubit_budget::json::JsonValueLimits;
use serde_json::Value;

use super::JsonTreeContext;
use super::JsonTreeProcessError;
use super::JsonTreeProcessor;
use super::JsonTreeVisitor;

/// Fully accounts materialized JSON trees using an internally owned budget.
#[must_use]
pub struct JsonTreeBudgetVisitor<R = JsonResource, Q = usize>
where
    Q: ResourceQuantity,
{
    budget: JsonValueBudget<R, Q>,
}

impl<R, Q> JsonTreeBudgetVisitor<R, Q>
where
    R: Clone,
    Q: ResourceQuantity,
{
    /// Creates a full-tree accounting object with fresh budget state.
    pub fn new(limits: JsonValueLimits<R, Q>) -> Self {
        Self {
            budget: JsonValueBudget::new(limits),
        }
    }

    /// Charges every node and payload represented by `value`.
    pub fn visit_tree(
        &mut self,
        value: &Value,
    ) -> Result<(), MeasuredBudgetError<R, Q>> {
        let mut transaction = self.budget.transaction();
        let result = JsonTreeProcessor::new(&mut transaction)
            .process(value, &mut NoopVisitor)
            .map_err(extract_budget);
        if result.is_ok() {
            transaction.commit();
        }
        result
    }

    /// Restores the owned budget to its original configured state.
    pub fn reset(&mut self) {
        self.budget.reset();
    }

    /// Returns the owned budget for read-only inspection.
    pub const fn budget(&self) -> &JsonValueBudget<R, Q> {
        &self.budget
    }

    /// Returns the owned budget for caller-managed accounting.
    pub fn budget_mut(&mut self) -> &mut JsonValueBudget<R, Q> {
        &mut self.budget
    }

    /// Consumes this visitor and returns its accumulated budget state.
    pub fn into_budget(self) -> JsonValueBudget<R, Q> {
        self.budget
    }
}

/// Does not add domain behavior while the processor performs full admission.
struct NoopVisitor;

impl JsonTreeVisitor for NoopVisitor {
    type Error = std::convert::Infallible;

    /// Accepts every admitted node.
    fn enter(
        &mut self,
        _value: &Value,
        _context: JsonTreeContext<'_>,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
}

/// Extracts the guaranteed infrastructure error from a no-op traversal.
fn extract_budget<R, Q>(
    error: JsonTreeProcessError<R, Q, std::convert::Infallible>,
) -> MeasuredBudgetError<R, Q>
where
    Q: ResourceQuantity,
{
    match error {
        JsonTreeProcessError::Budget(error) => error,
        JsonTreeProcessError::Visitor(error) => match error {},
    }
}
