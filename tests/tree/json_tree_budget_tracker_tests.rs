// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use qubit_budget::json::JsonValueLimits;
use qubit_json::tree::JsonTreeBudgetTracker;
use serde_json::json;

/// Verifies that a tracker accounts a materialized JSON tree.
#[test]
fn test_budget_tracker_accounts_a_materialized_tree() {
    let mut tracker =
        JsonTreeBudgetTracker::new(JsonValueLimits::empty().with_max_nodes(2));

    tracker
        .account(&json!({"ok": true}))
        .expect("two nodes fit");
    assert_eq!(tracker.budget().used_nodes(), Some(2));
}

/// Verifies that tracker budget accessors expose and transfer state.
#[test]
fn test_budget_tracker_exposes_and_transfers_budget_state() {
    let mut tracker = JsonTreeBudgetTracker::new(JsonValueLimits::empty());
    assert_eq!(tracker.budget().used_nodes(), None);
    tracker
        .account(&json!({"key": true}))
        .expect("the unrestricted budget should admit the object");
    assert_eq!(tracker.budget_mut().used_nodes(), None);

    let budget = tracker.into_budget();
    assert_eq!(budget.used_nodes(), None);
}

/// Verifies failed accounting rolls back and reset clears prior admissions.
#[test]
fn test_budget_tracker_rolls_back_rejection_and_resets() {
    let mut tracker =
        JsonTreeBudgetTracker::new(JsonValueLimits::empty().with_max_nodes(1));

    tracker.account(&json!(true)).expect("one node should fit");
    assert!(tracker.account(&json!([true])).is_err());
    assert_eq!(tracker.budget().used_nodes(), Some(1));

    tracker.reset();
    assert_eq!(tracker.budget().used_nodes(), Some(0));
}
