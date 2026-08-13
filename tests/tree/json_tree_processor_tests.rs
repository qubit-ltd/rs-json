// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use qubit_budget::json::JsonValueBudget;
use qubit_budget::json::JsonValueLimits;
use qubit_json::tree::JsonTreeContext;
use qubit_json::tree::JsonTreeLocation;
use qubit_json::tree::JsonTreeProcessor;
use qubit_json::tree::JsonTreeVisitor;
use serde_json::Value;
use serde_json::json;

/// Records the context received by a tree visitor.
struct RecordingVisitor {
    events: Vec<String>,
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
    let mut budget = JsonValueBudget::new(JsonValueLimits::empty());
    let mut visitor = RecordingVisitor { events: Vec::new() };

    JsonTreeProcessor::new(&mut budget)
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

/// Verifies that the root context is explicitly represented.
#[test]
fn test_context_root_location_is_distinct() {
    assert_eq!(JsonTreeLocation::Root, JsonTreeLocation::Root);
}
