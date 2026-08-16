// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Traverses materialized JSON trees with explicit resource accounting.

mod json_tree_budget_rejection;
mod json_tree_budget_tracker;
mod json_tree_context;
mod json_tree_control;
mod json_tree_location;
mod json_tree_mut_visitor;
mod json_tree_mutator;
mod json_tree_process_error;
mod json_tree_reader;
mod json_tree_visitor;

/// Reject reason when traversal exceeds configured JSON budget constraints.
pub use json_tree_budget_rejection::JsonTreeBudgetRejection;
/// Tracks total budget use for a full materialized JSON tree.
pub use json_tree_budget_tracker::JsonTreeBudgetTracker;
/// Encodes current traversal position for callback and accounting context.
pub use json_tree_context::JsonTreeContext;
/// Controls whether traversal descends, skips, or stops.
pub use json_tree_control::JsonTreeControl;
/// Materialized JSON location descriptor used during traversal.
pub use json_tree_location::JsonTreeLocation;
/// Visitor callback trait for mutable tree traversal.
pub use json_tree_mut_visitor::JsonTreeMutVisitor;
/// Mutable traversal helper for budget-aware tree mutation.
pub use json_tree_mutator::JsonTreeMutator;
/// Error envelope for budgeted tree traversal failures.
pub use json_tree_process_error::JsonTreeProcessError;
/// Read-only tree traversal entrypoint with budget enforcement.
pub use json_tree_reader::JsonTreeReader;
/// Read-only visitor callback trait for tree traversal.
pub use json_tree_visitor::JsonTreeVisitor;
