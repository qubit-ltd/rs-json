// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Traverses materialized JSON trees with explicit resource accounting.

mod internal;
mod json_tree_budget_tracker;
mod json_tree_context;
mod json_tree_control;
mod json_tree_location;
mod json_tree_mut_visitor;
mod json_tree_mutate_error;
mod json_tree_mutator;
mod json_tree_process_error;
mod json_tree_reader;
mod json_tree_visitor;

pub use json_tree_budget_tracker::JsonTreeBudgetTracker;
pub use json_tree_context::JsonTreeContext;
pub use json_tree_control::JsonTreeControl;
pub use json_tree_location::JsonTreeLocation;
pub use json_tree_mut_visitor::JsonTreeMutVisitor;
pub use json_tree_mutate_error::JsonTreeMutateError;
pub use json_tree_mutator::JsonTreeMutator;
pub use json_tree_process_error::JsonTreeProcessError;
pub use json_tree_reader::JsonTreeReader;
pub use json_tree_visitor::JsonTreeVisitor;
