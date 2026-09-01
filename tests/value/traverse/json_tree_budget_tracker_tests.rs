// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use qubit_budget::json::JsonResource;
use qubit_budget::json::JsonValueLimits;
use qubit_json::value::traverse::JsonTreeBudgetTracker;
use serde_json::json;

/// Verifies that a tracker accounts a materialized JSON tree.
#[test]
fn test_budget_tracker_accounts_a_materialized_tree() {
    let mut tracker = JsonTreeBudgetTracker::new(
        JsonValueLimits::<JsonResource, usize>::builder()
            .max_depth(3)
            .max_nodes(4)
            .max_sequence_items(2)
            .max_map_entries(1)
            .max_key_bytes(1)
            .max_string_bytes(2)
            .max_number_bytes(1)
            .max_payload_bytes(4)
            .build(),
    );

    tracker
        .account(&json!({"a": [1, "bc"]}))
        .expect("the complete tree should fit every exact limit");
    assert_eq!(tracker.budget().used_nodes(), Some(4));
    assert_eq!(tracker.budget().used_payload_bytes(), Some(4));
}

/// Verifies that tracker budget accessors expose and transfer state.
#[test]
fn test_budget_tracker_exposes_and_transfers_budget_state() {
    let mut tracker = JsonTreeBudgetTracker::new(JsonValueLimits::<JsonResource, usize>::builder().build());
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
        JsonTreeBudgetTracker::new(JsonValueLimits::<JsonResource, usize>::builder().max_nodes(1).build());

    tracker.account(&json!(true)).expect("one node should fit");
    assert!(tracker.account(&json!([true])).is_err());
    assert_eq!(tracker.budget().used_nodes(), Some(1));

    tracker.reset();
    assert_eq!(tracker.budget().used_nodes(), Some(0));
}
