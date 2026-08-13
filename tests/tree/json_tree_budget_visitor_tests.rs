// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use qubit_budget::ResourceLimit;
use qubit_budget::StructureLimits;
use qubit_budget::json::JsonResource;
use qubit_budget::json::JsonValueLimits;
use qubit_json::tree::JsonTreeBudgetVisitor;
use serde_json::json;

/// Verifies that visits accumulate and reset restores the configured budget.
#[test]
fn test_visit_tree_accumulates_and_reset_clears_usage() {
    let limits = JsonValueLimits::empty()
        .with_structure_limits(StructureLimits::empty().with_nodes_limit(
            ResourceLimit::new(JsonResource::Nodes, 2_usize),
        ));
    let mut visitor = JsonTreeBudgetVisitor::new(limits);

    visitor.visit_tree(&json!(true)).expect("first scalar fits");
    visitor
        .visit_tree(&json!(false))
        .expect("second scalar fits");
    assert!(visitor.visit_tree(&json!(null)).is_err());
    visitor.reset();
    visitor
        .visit_tree(&json!(null))
        .expect("reset clears prior usage");
}
