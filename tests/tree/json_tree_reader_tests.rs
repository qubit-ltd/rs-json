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

    fn enter(
        &mut self,
        _value: &Value,
        _context: JsonTreeContext<'_>,
    ) -> Result<(), Self::Error> {
        Err("stop")
    }

    fn leave(
        &mut self,
        _value: &Value,
        _context: JsonTreeContext<'_>,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl JsonTreeVisitor for RecordingVisitor {
    type Error = std::convert::Infallible;

    fn enter(
        &mut self,
        value: &Value,
        context: JsonTreeContext<'_>,
    ) -> Result<(), Self::Error> {
        self.events.push(format!(
            "enter:{:?}:{}:{value}",
            context.location, context.depth
        ));
        Ok(())
    }

    fn leave(
        &mut self,
        value: &Value,
        context: JsonTreeContext<'_>,
    ) -> Result<(), Self::Error> {
        self.events.push(format!(
            "leave:{:?}:{}:{value}",
            context.location, context.depth
        ));
        Ok(())
    }
}

/// Verifies preorder/postorder traversal and object-key locations.
#[test]
fn test_process_visits_depth_first_with_root_and_key_locations() {
    let value = json!({"a": [true]});
    let mut budget = JsonValueLimits::<JsonResource, usize>::builder()
        .build()
        .budget();
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
