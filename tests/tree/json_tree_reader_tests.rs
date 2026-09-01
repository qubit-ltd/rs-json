// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use qubit_budget::json::JsonResource;
use qubit_budget::json::JsonValueLimits;
use qubit_json::value::traverse::JsonTreeContext;
use qubit_json::value::traverse::JsonTreeLocation;
use qubit_json::value::traverse::JsonTreeProcessError;
use qubit_json::value::traverse::JsonTreeReader;
use qubit_json::value::traverse::JsonTreeVisitor;
use serde_json::Value;
use serde_json::json;

/// Records the context received by a tree visitor.
struct RecordingVisitor {
    events: Vec<String>,
}

/// Stops traversal to verify the caller-owned transaction rolls back.
struct FailingVisitor;

impl JsonTreeVisitor for FailingVisitor {
    type Error = &'static str;

    fn enter(&mut self, _value: &Value, _context: JsonTreeContext<'_>) -> Result<(), Self::Error> {
        Err("stop")
    }

    fn leave(&mut self, _value: &Value, _context: JsonTreeContext<'_>) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl JsonTreeVisitor for RecordingVisitor {
    type Error = std::convert::Infallible;

    fn enter(&mut self, value: &Value, context: JsonTreeContext<'_>) -> Result<(), Self::Error> {
        self.events
            .push(format!("enter:{:?}:{}:{value}", context.location, context.depth));
        Ok(())
    }

    fn leave(&mut self, value: &Value, context: JsonTreeContext<'_>) -> Result<(), Self::Error> {
        self.events
            .push(format!("leave:{:?}:{}:{value}", context.location, context.depth));
        Ok(())
    }
}

/// Verifies preorder/postorder traversal and object-key locations.
#[test]
fn test_process_visits_depth_first_with_root_and_key_locations() {
    let value = json!({"a": [true]});
    let mut budget = JsonValueLimits::<JsonResource, usize>::builder().build().budget();
    let mut transaction = budget.transaction();
    let mut visitor = RecordingVisitor { events: Vec::new() };

    JsonTreeReader::new(&mut transaction)
        .process(&value, &mut visitor)
        .expect("unlimited tree processing succeeds");

    assert_eq!(
        visitor.events,
        [
            "enter:Root:1:{\"a\":[true]}",
            "enter:ObjectValue { key: \"a\" }:2:[true]",
            "enter:ArrayElement { index: 0 }:3:true",
            "leave:ArrayElement { index: 0 }:3:true",
            "leave:ObjectValue { key: \"a\" }:2:[true]",
            "leave:Root:1:{\"a\":[true]}",
        ],
    );
}

/// Verifies the unlimited fast path preserves every visitor callback emitted
/// by bounded traversal.
#[test]
fn test_unlimited_and_bounded_process_emit_identical_callbacks() {
    let value = json!({"a": [true, {"b": null}]});
    let mut unlimited_budget = JsonValueLimits::<JsonResource, usize>::new().budget();
    let mut unlimited_transaction = unlimited_budget.transaction();
    let mut unlimited_visitor = RecordingVisitor { events: Vec::new() };
    JsonTreeReader::new(&mut unlimited_transaction)
        .process(&value, &mut unlimited_visitor)
        .expect("unlimited traversal succeeds");

    let mut bounded_budget = JsonValueLimits::<JsonResource, usize>::builder()
        .max_nodes(8)
        .build()
        .budget();
    let mut bounded_transaction = bounded_budget.transaction();
    let mut bounded_visitor = RecordingVisitor { events: Vec::new() };
    JsonTreeReader::new(&mut bounded_transaction)
        .process(&value, &mut bounded_visitor)
        .expect("bounded traversal succeeds");

    assert_eq!(unlimited_visitor.events, bounded_visitor.events);
    assert_eq!(unlimited_transaction.used_nodes(), None);
    assert_eq!(bounded_transaction.used_nodes(), Some(5));
}

/// Verifies that a processor can outlive each JSON value it processes.
#[test]
fn test_process_accepts_value_borrowed_shorter_than_budget() {
    let mut budget = JsonValueLimits::<JsonResource, usize>::builder()
        .max_nodes(2)
        .build()
        .budget();
    let mut transaction = budget.transaction();
    let mut visitor = RecordingVisitor { events: Vec::new() };

    {
        let mut processor = JsonTreeReader::new(&mut transaction);
        let value = json!([true]);
        processor
            .process(&value, &mut visitor)
            .expect("short-lived JSON value processing succeeds");
    }

    assert_eq!(transaction.used_nodes(), Some(2));
}

/// Verifies a visitor error leaves the caller's committed budget unchanged.
#[test]
fn test_process_rolls_back_transaction_when_visitor_returns_error() {
    let mut budget = JsonValueLimits::<JsonResource, usize>::builder()
        .max_nodes(4)
        .build()
        .budget();

    {
        let mut transaction = budget.transaction();
        let error = JsonTreeReader::new(&mut transaction)
            .process(&json!([Value::Null]), &mut FailingVisitor)
            .expect_err("visitor must stop traversal");
        assert!(matches!(error, JsonTreeProcessError::Visitor("stop")));
    }

    assert_eq!(budget.used_nodes(), Some(0));
}

/// Verifies that the root context is explicitly represented.
#[test]
fn test_context_root_location_is_distinct() {
    assert_eq!(JsonTreeLocation::Root, JsonTreeLocation::Root);
}

/// Verifies account stages a complete tree but leaves commit control with the
/// caller-owned transaction.
#[test]
fn test_account_stages_exact_charges_and_rolls_back_without_commit() {
    let limits = JsonValueLimits::<JsonResource, usize>::builder()
        .max_nodes(3)
        .max_payload_bytes(16)
        .build();
    let mut budget = limits.budget();

    {
        let mut transaction = budget.transaction();
        JsonTreeReader::new(&mut transaction)
            .account(&json!({"a": ["bc"]}))
            .expect("tree should fit");
        assert_eq!(transaction.used_nodes(), Some(3));
        assert_eq!(transaction.used_payload_bytes(), Some(3));
    }

    assert_eq!(budget.used_nodes(), Some(0));
    assert_eq!(budget.used_payload_bytes(), Some(0));
}

/// Verifies an account rejection returns the raw budget error and poisons the
/// caller-owned transaction.
#[test]
fn test_account_returns_budget_error_and_poisons_transaction() {
    let mut budget = JsonValueLimits::<JsonResource, usize>::builder()
        .max_nodes(1)
        .max_sequence_items(0)
        .build()
        .budget();
    let mut transaction = budget.transaction();
    let mut reader = JsonTreeReader::new(&mut transaction);

    let first_error = reader
        .account(&json!([true]))
        .expect_err("array item limit rejects the tree");
    let repeated_error = reader
        .account(&Value::Null)
        .expect_err("poisoned transaction rejects a smaller tree");
    assert_eq!(repeated_error.resource(), first_error.resource());
    let commit_error = transaction
        .commit()
        .expect_err("poisoned tree transaction cannot commit");
    assert_eq!(commit_error.resource(), first_error.resource());
    assert_eq!(budget.used_nodes(), Some(0));
}
