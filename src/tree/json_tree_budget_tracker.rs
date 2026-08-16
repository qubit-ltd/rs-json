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
use super::JsonTreeReader;
use super::JsonTreeVisitor;

/// Fully accounts materialized JSON trees using an internally owned budget.
#[must_use]
pub struct JsonTreeBudgetTracker<R = JsonResource, Q = usize>
where
    Q: ResourceQuantity,
{
    budget: JsonValueBudget<R, Q>,
}

impl<R, Q> JsonTreeBudgetTracker<R, Q>
where
    R: Clone,
    Q: ResourceQuantity,
{
    /// Creates a full-tree tracker with fresh budget state.
    ///
    /// # Parameters
    ///
    /// * `limits` - Resource limits used by the owned budget.
    ///
    /// # Returns
    ///
    /// A tracker initialized with the supplied limits.
    pub fn new(limits: JsonValueLimits<R, Q>) -> Self {
        Self {
            budget: JsonValueBudget::new(limits),
        }
    }

    /// Charges every node and payload represented by `value`.
    ///
    /// # Parameters
    ///
    /// * `value` - JSON tree whose resources are charged.
    ///
    /// # Returns
    ///
    /// `Ok(())` when the complete tree is admitted.
    ///
    /// # Errors
    ///
    /// Returns the first measured budget rejection encountered while walking
    /// the tree. Charges are committed only when the complete walk succeeds.
    pub fn account(
        &mut self,
        value: &Value,
    ) -> Result<(), MeasuredBudgetError<R, Q>> {
        let mut transaction = self.budget.transaction();
        let result = JsonTreeReader::new(&mut transaction)
            .process(value, &mut NoopVisitor)
            .map_err(Self::extract_budget);
        if result.is_ok() {
            transaction.commit();
        }
        result
    }

    /// Restores the owned budget to its original configured state.
    ///
    /// This clears accumulated charges and makes the tracker ready for a new
    /// independent accounting run.
    pub fn reset(&mut self) {
        self.budget.reset();
    }

    /// Returns the owned budget for read-only inspection.
    ///
    /// # Returns
    ///
    /// A shared reference to the accumulated budget state.
    pub const fn budget(&self) -> &JsonValueBudget<R, Q> {
        &self.budget
    }

    /// Returns the owned budget for caller-managed accounting.
    ///
    /// # Returns
    ///
    /// A mutable reference to the accumulated budget state.
    pub fn budget_mut(&mut self) -> &mut JsonValueBudget<R, Q> {
        &mut self.budget
    }

    /// Consumes this tracker and returns its accumulated budget state.
    ///
    /// # Returns
    ///
    /// The owned budget, including all charges accumulated by this tracker.
    pub fn into_budget(self) -> JsonValueBudget<R, Q> {
        self.budget
    }

    /// Extracts the infrastructure error from a no-op traversal.
    fn extract_budget(
        error: JsonTreeProcessError<R, Q, std::convert::Infallible>,
    ) -> MeasuredBudgetError<R, Q> {
        match error {
            JsonTreeProcessError::Budget(error) => error,
            JsonTreeProcessError::Visitor(error) => match error {},
        }
    }
}

/// Does not add domain behavior while the reader performs full admission.
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
