// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines failures produced while mutating JSON trees.

use std::fmt::Debug;

use qubit_budget::MeasuredBudgetError;
use thiserror::Error;

/// Identifies the phase that failed during a budget-aware JSON tree mutation.
///
/// Input and output budgets are deliberately separate so callers can decide
/// which transaction, if any, remains eligible for commit after a failure.
///
/// # Type Parameters
///
/// * `R` - Resource identity attached to budget failures.
/// * `Q` - Quantity representation attached to budget failures.
/// * `E` - Error type returned by the mutation visitor.
#[derive(Debug, Error)]
pub enum JsonTreeMutateError<R, Q, E>
where
    Q: Copy + Debug,
{
    /// The complete tree before mutation exceeded its input budget.
    #[error("JSON tree input budget rejected the original value")]
    InputBudget(#[source] MeasuredBudgetError<R, Q>),
    /// The caller-defined visitor failed after mutation began.
    #[error("JSON tree mutation visitor failed")]
    Visitor(E),
    /// The complete tree after mutation exceeded its output budget.
    #[error("JSON tree output budget rejected the mutated value")]
    OutputBudget(#[source] MeasuredBudgetError<R, Q>),
}
