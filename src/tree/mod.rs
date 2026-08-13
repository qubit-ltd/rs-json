// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Traverses materialized JSON trees with explicit resource accounting.

mod json_budget_rejection;
mod json_tree_budget_visitor;
mod json_tree_context;
mod json_tree_control;
mod json_tree_location;
mod json_tree_mut_visitor;
mod json_tree_process_error;
mod json_tree_processor;
mod json_tree_visitor;

pub use json_budget_rejection::JsonBudgetRejection;
pub use json_tree_budget_visitor::JsonTreeBudgetVisitor;
pub use json_tree_context::JsonTreeContext;
pub use json_tree_control::JsonTreeControl;
pub use json_tree_location::JsonTreeLocation;
pub use json_tree_mut_visitor::JsonTreeMutVisitor;
pub use json_tree_process_error::JsonTreeProcessError;
pub use json_tree_processor::JsonTreeProcessor;
pub use json_tree_visitor::JsonTreeVisitor;

pub use crate::budget::BudgetedJsonValueSeed as BudgetedValueSeed;
