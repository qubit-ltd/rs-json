// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests mutable JSON tree traversal, including accounting, replacement, and
//! restoration behavior after visitor errors or panics.

use std::panic::AssertUnwindSafe;
use std::panic::catch_unwind;

use qubit_budget::json::JsonResource;
use qubit_budget::json::JsonValueLimits;
use qubit_json::value::traverse::JsonTreeContext;
use qubit_json::value::traverse::JsonTreeControl;
use qubit_json::value::traverse::JsonTreeLocation;
use qubit_json::value::traverse::JsonTreeMutVisitor;
use qubit_json::value::traverse::JsonTreeMutateError;
use qubit_json::value::traverse::JsonTreeMutator;
use qubit_json::value::traverse::JsonTreeReader;
use serde_json::Value;
use serde_json::json;
use serde_json::to_string;

/// Builds limits that expose exact cumulative accounting in tests.
fn measured_limits() -> JsonValueLimits<JsonResource, usize> {
    JsonValueLimits::builder()
        .max_nodes(128)
        .max_payload_bytes(1_024)
        .build()
}

/// Counts every callback while allowing traversal to descend through the tree.
struct CountingVisitor {
    calls: usize,
}

impl JsonTreeMutVisitor for CountingVisitor {
    type Error = std::convert::Infallible;

    fn visit(&mut self, _value: &mut Value, _context: JsonTreeContext<'_>) -> Result<JsonTreeControl, Self::Error> {
        self.calls += 1;
        Ok(JsonTreeControl::Descend)
    }
}

/// Replaces depth-one nodes with null and skips each replaced subtree.
struct ShrinkingVisitor;

impl JsonTreeMutVisitor for ShrinkingVisitor {
    type Error = std::convert::Infallible;

    fn visit(&mut self, value: &mut Value, context: JsonTreeContext<'_>) -> Result<JsonTreeControl, Self::Error> {
        if context.depth == 1 {
            *value = Value::Null;
        }
        Ok(JsonTreeControl::SkipSubtree)
    }
}

/// Replaces the visited node with a nested value and skips its callbacks.
struct SkipWithExpandedReplacement;

impl JsonTreeMutVisitor for SkipWithExpandedReplacement {
    type Error = std::convert::Infallible;

    fn visit(&mut self, value: &mut Value, _context: JsonTreeContext<'_>) -> Result<JsonTreeControl, Self::Error> {
        *value = json!({"added": [null]});
        Ok(JsonTreeControl::SkipSubtree)
    }
}

/// Mutates the first visited node, then mutates and fails on the next callback.
struct FailingVisitor {
    calls: usize,
}

/// Adds one descendant per callback and stops after eight additions.
struct BoundedExpansionVisitor {
    additions: usize,
}

impl JsonTreeMutVisitor for BoundedExpansionVisitor {
    type Error = &'static str;

    fn visit(&mut self, value: &mut Value, _context: JsonTreeContext<'_>) -> Result<JsonTreeControl, Self::Error> {
        if self.additions == 8 {
            return Err("expansion limit reached");
        }
        self.additions += 1;
        *value = json!([null]);
        Ok(JsonTreeControl::Descend)
    }
}

impl JsonTreeMutVisitor for FailingVisitor {
    type Error = &'static str;

    fn visit(&mut self, value: &mut Value, _context: JsonTreeContext<'_>) -> Result<JsonTreeControl, Self::Error> {
        self.calls += 1;
        if self.calls == 1 {
            *value = json!({"changed": [1, 2]});
            Ok(JsonTreeControl::Descend)
        } else {
            *value = json!("partially changed");
            Err("stop")
        }
    }
}

/// Marks visited objects and panics once the configured callback count is met.
struct PanickingVisitor {
    calls: usize,
    panic_after: usize,
}

/// Replaces nested object and array children while recording later visits.
#[derive(Default)]
struct StructuralReplacementVisitor {
    /// Ordered event names used to check traversal resumption after
    /// replacement.
    events: Vec<&'static str>,
    /// Whether traversal entered a descendant created by the object-child
    /// replacement.
    visited_object_replacement_child: bool,
    /// Whether traversal entered a descendant created by the array-child
    /// replacement.
    visited_array_replacement_child: bool,
    /// Whether traversal resumed the original object's next sibling.
    visited_object_sibling: bool,
    /// Whether traversal resumed the original array's next sibling.
    visited_array_sibling: bool,
    /// Whether traversal resumed the root object's final sibling.
    visited_root_sibling: bool,
}

impl JsonTreeMutVisitor for StructuralReplacementVisitor {
    type Error = std::convert::Infallible;

    /// Replaces two nested children and records descendants and siblings that
    /// must remain reachable afterward.
    fn visit(&mut self, value: &mut Value, context: JsonTreeContext<'_>) -> Result<JsonTreeControl, Self::Error> {
        match context.location {
            JsonTreeLocation::ObjectValue {
                key: "a_replace_object",
            } if context.depth == 3 => {
                *value = json!(["object replacement child"]);
                self.events.push("a_replace_object");
            }
            JsonTreeLocation::ArrayElement { index: 0 } if context.depth == 3 => {
                *value = json!({"array replacement child": null});
            }
            JsonTreeLocation::ObjectValue { key: "z_after_object" } => {
                self.visited_object_sibling = true;
                self.events.push("z_after_object");
            }
            JsonTreeLocation::ArrayElement { index: 1 } if context.depth == 3 => {
                self.visited_array_sibling = true;
            }
            JsonTreeLocation::ObjectValue { key: "last" } => {
                self.visited_root_sibling = true;
            }
            JsonTreeLocation::ArrayElement { index: 0 }
                if context.depth == 4 && value == "object replacement child" =>
            {
                self.visited_object_replacement_child = true;
            }
            JsonTreeLocation::ObjectValue {
                key: "array replacement child",
            } if context.depth == 4 => {
                self.visited_array_replacement_child = true;
            }
            JsonTreeLocation::Root | JsonTreeLocation::ArrayElement { .. } | JsonTreeLocation::ObjectValue { .. } => {}
        }
        Ok(JsonTreeControl::Descend)
    }
}

impl JsonTreeMutVisitor for PanickingVisitor {
    type Error = std::convert::Infallible;

    fn visit(&mut self, value: &mut Value, _context: JsonTreeContext<'_>) -> Result<JsonTreeControl, Self::Error> {
        self.calls += 1;
        if let Value::Object(entries) = value {
            entries.insert("visited".to_owned(), json!(true));
        }
        assert!(self.calls < self.panic_after, "visitor panic regression");
        Ok(JsonTreeControl::Descend)
    }
}

/// Verifies an input rejection happens before the first visitor mutation.
#[test]
fn test_process_rejects_input_before_mutation() {
    let input_limits = JsonValueLimits::<JsonResource, usize>::builder().max_nodes(1).build();
    let mut input_budget = input_limits.budget();
    let mut output_budget = measured_limits().budget();
    let mut input = input_budget.transaction();
    let mut output = output_budget.transaction();
    let mut visitor = CountingVisitor { calls: 0 };
    let mut value = json!([null]);
    let original = value.clone();

    let error = JsonTreeMutator::new(&mut input, &mut output)
        .process(&mut value, &mut visitor)
        .expect_err("the complete input tree must exceed one node");

    assert!(matches!(
        error,
        JsonTreeMutateError::InputBudget(error)
            if error.resource() == &JsonResource::Nodes
    ));
    assert_eq!(visitor.calls, 0);
    assert_eq!(value, original);
    assert_eq!(output.used_nodes(), Some(0));
}

#[test]
fn test_mutator_visitor_can_bound_work_from_added_descendants() {
    let limits = measured_limits();
    let mut input_budget = limits.budget();
    let mut output_budget = limits.budget();
    let mut input = input_budget.transaction();
    let mut output = output_budget.transaction();
    let mut value = Value::Null;
    let mut visitor = BoundedExpansionVisitor { additions: 0 };

    let error = JsonTreeMutator::new(&mut input, &mut output)
        .process(&mut value, &mut visitor)
        .expect_err("visitor must stop after its configured number of additions");

    assert!(matches!(error, JsonTreeMutateError::Visitor("expansion limit reached")));
    assert_eq!(visitor.additions, 8);
    assert_eq!(value, json!([[[[[[[[null]]]]]]]]));
}

/// Verifies shrinking transformations account the complete original and the
/// smaller final tree independently.
#[test]
fn test_process_accounts_input_and_shrunken_output_separately() {
    let mut input_budget = measured_limits().budget();
    let mut output_budget = measured_limits().budget();
    let mut input = input_budget.transaction();
    let mut output = output_budget.transaction();
    let mut value = json!(["long", null]);

    JsonTreeMutator::new(&mut input, &mut output)
        .process(&mut value, &mut ShrinkingVisitor)
        .expect("both complete trees must fit their independent budgets");

    assert_eq!(value, Value::Null);
    assert_eq!(input.used_nodes(), Some(3));
    assert_eq!(output.used_nodes(), Some(1));
    assert_eq!(input.used_payload_bytes(), Some(4));
    assert_eq!(output.used_payload_bytes(), Some(0));
}

/// Verifies output accounting traverses replacement descendants even when the
/// visitor skips their callbacks.
#[test]
fn test_process_accounts_skipped_replacement_subtree() {
    let mut input_budget = measured_limits().budget();
    let mut output_budget = measured_limits().budget();
    let mut input = input_budget.transaction();
    let mut output = output_budget.transaction();
    let mut value = Value::Null;

    JsonTreeMutator::new(&mut input, &mut output)
        .process(&mut value, &mut SkipWithExpandedReplacement)
        .expect("the expanded replacement must fit generous output limits");

    assert_eq!(value, json!({"added": [null]}));
    assert_eq!(input.used_nodes(), Some(1));
    assert_eq!(output.used_nodes(), Some(3));
    assert_eq!(output.used_payload_bytes(), Some(5));
}

/// Verifies visitor failure retains partial mutation without starting output
/// accounting.
#[test]
fn test_process_preserves_mutation_when_visitor_fails() {
    let mut input_budget = measured_limits().budget();
    let mut output_budget = measured_limits().budget();
    let mut input = input_budget.transaction();
    let mut output = output_budget.transaction();
    let mut value = json!({"original": true});

    let error = JsonTreeMutator::new(&mut input, &mut output)
        .process(&mut value, &mut FailingVisitor { calls: 0 })
        .expect_err("the visitor deliberately fails");

    assert!(matches!(error, JsonTreeMutateError::Visitor("stop")));
    assert_eq!(value, json!({"changed": "partially changed"}));
    assert_eq!(
        to_string(&value).expect("partially mutated value serializes"),
        r#"{"changed":"partially changed"}"#,
    );
    assert_eq!(input.used_nodes(), Some(2));
    assert_eq!(output.used_nodes(), Some(0));
}

/// Verifies reader, mutation input, and mutation output use identical static
/// tree measurements.
#[test]
fn test_reader_and_mutator_measure_the_same_static_tree() {
    let limits = measured_limits();
    let value = json!({"name": "qubit", "values": [1.25, true, null]});
    let mut read_budget = limits.budget();
    let mut read = read_budget.transaction();
    JsonTreeReader::new(&mut read)
        .account(&value)
        .expect("reader should admit the fixture");

    let mut input_budget = limits.budget();
    let mut output_budget = limits.budget();
    let mut input = input_budget.transaction();
    let mut output = output_budget.transaction();
    let mut mutable_value = value.clone();
    JsonTreeMutator::new(&mut input, &mut output)
        .process(&mut mutable_value, &mut CountingVisitor { calls: 0 })
        .expect("mutator should admit the unchanged fixture");

    assert_eq!(input.used_nodes(), read.used_nodes());
    assert_eq!(output.used_nodes(), read.used_nodes());
    assert_eq!(input.used_payload_bytes(), read.used_payload_bytes());
    assert_eq!(output.used_payload_bytes(), read.used_payload_bytes());
}

/// Verifies a panic leaves the mutable root reassembled and serializable.
#[test]
fn test_process_restores_root_after_visitor_panic() {
    let mut input_budget = measured_limits().budget();
    let mut output_budget = measured_limits().budget();
    let mut input = input_budget.transaction();
    let mut output = output_budget.transaction();
    let mut value = json!({"first": [1, 2], "second": true});

    let result = catch_unwind(AssertUnwindSafe(|| {
        let _ = JsonTreeMutator::new(&mut input, &mut output).process(
            &mut value,
            &mut PanickingVisitor {
                calls: 0,
                panic_after: 3,
            },
        );
    }));

    assert!(result.is_err());
    assert_eq!(value["visited"], Value::Bool(true));
    assert!(value["first"].is_array());
    assert_eq!(output.used_nodes(), Some(0));
    assert_eq!(
        to_string(&value).expect("panic-restored value serializes"),
        r#"{"first":[1,2],"second":true,"visited":true}"#,
    );
}

/// Verifies suspended parent cursors remain valid when a nested child changes
/// between object, array, and scalar structures.
#[test]
fn test_process_continues_after_nested_child_structure_replacement() {
    let mut input_budget = measured_limits().budget();
    let mut output_budget = measured_limits().budget();
    let mut input = input_budget.transaction();
    let mut output = output_budget.transaction();
    let mut value = json!({
        "first": {"a_replace_object": true, "z_after_object": 1},
        "middle": [{"replace_array": true}, "after array"],
        "last": "after root",
    });
    let mut visitor = StructuralReplacementVisitor::default();

    JsonTreeMutator::new(&mut input, &mut output)
        .process(&mut value, &mut visitor)
        .expect("nested structural replacements must preserve parent cursors");

    assert_eq!(
        value,
        json!({
            "first": {"a_replace_object": ["object replacement child"], "z_after_object": 1},
            "middle": [{"array replacement child": null}, "after array"],
            "last": "after root",
        }),
    );
    assert!(visitor.visited_object_replacement_child);
    assert!(visitor.visited_array_replacement_child);
    assert!(visitor.visited_object_sibling);
    assert!(visitor.visited_array_sibling);
    assert!(visitor.visited_root_sibling);
    let replaced = visitor
        .events
        .iter()
        .position(|event| *event == "a_replace_object")
        .unwrap();
    let after = visitor
        .events
        .iter()
        .position(|event| *event == "z_after_object")
        .unwrap();
    assert!(
        replaced < after,
        "the parent cursor must resume after the replaced child"
    );
    assert_eq!(
        visitor
            .events
            .iter()
            .filter(|event| **event == "z_after_object")
            .count(),
        1
    );
}

/// Verifies nested structural mutations remain observable when output
/// accounting rejects the completed result.
#[test]
fn test_process_retains_nested_mutations_after_output_budget_failure() {
    let mut input_budget = measured_limits().budget();
    let mut output_budget = JsonValueLimits::<JsonResource, usize>::builder()
        .max_nodes(5)
        .max_payload_bytes(1_024)
        .build()
        .budget();
    let mut input = input_budget.transaction();
    let mut output = output_budget.transaction();
    let mut value = json!({
        "first": {"a_replace_object": true, "z_after_object": 1},
        "middle": [{"replace_array": true}, "after array"],
        "last": "after root",
    });
    let mut visitor = StructuralReplacementVisitor::default();

    let error = JsonTreeMutator::new(&mut input, &mut output)
        .process(&mut value, &mut visitor)
        .expect_err("the expanded result must exceed the output node budget");

    assert!(matches!(
        error,
        JsonTreeMutateError::OutputBudget(error)
            if error.resource() == &JsonResource::Nodes
    ));
    assert_eq!(
        value,
        json!({
            "first": {"a_replace_object": ["object replacement child"], "z_after_object": 1},
            "middle": [{"array replacement child": null}, "after array"],
            "last": "after root",
        }),
    );
    assert!(visitor.visited_object_replacement_child);
    assert!(visitor.visited_array_replacement_child);
    assert!(visitor.visited_object_sibling);
    assert!(visitor.visited_array_sibling);
    assert!(visitor.visited_root_sibling);
    let replaced = visitor
        .events
        .iter()
        .position(|event| *event == "a_replace_object")
        .unwrap();
    let after = visitor
        .events
        .iter()
        .position(|event| *event == "z_after_object")
        .unwrap();
    assert!(
        replaced < after,
        "the parent cursor must resume after the replaced child"
    );
    assert_eq!(
        visitor
            .events
            .iter()
            .filter(|event| **event == "z_after_object")
            .count(),
        1
    );
}

/// Verifies both accounting passes and mutable callbacks avoid Rust recursion.
#[test]
fn test_process_handles_deep_tree_without_rust_recursion() {
    const CONTAINER_DEPTH: usize = 512;

    let limits = JsonValueLimits::<JsonResource, usize>::builder()
        .max_depth(CONTAINER_DEPTH + 1)
        .max_nodes(CONTAINER_DEPTH + 1)
        .build();
    let mut value = Value::Null;
    for _ in 0..CONTAINER_DEPTH {
        value = Value::Array(vec![value]);
    }
    let mut input_budget = limits.budget();
    let mut output_budget = limits.budget();
    let mut input = input_budget.transaction();
    let mut output = output_budget.transaction();
    let mut visitor = CountingVisitor { calls: 0 };

    JsonTreeMutator::new(&mut input, &mut output)
        .process(&mut value, &mut visitor)
        .expect("deep input and output trees must use explicit traversal stacks");

    assert_eq!(visitor.calls, CONTAINER_DEPTH + 1);
    assert_eq!(input.used_nodes(), Some(CONTAINER_DEPTH + 1));
    assert_eq!(output.used_nodes(), Some(CONTAINER_DEPTH + 1));

    for _ in 0..CONTAINER_DEPTH {
        let Value::Array(mut items) = value else {
            panic!("the deep fixture must retain one array at every level");
        };
        value = items.pop().expect("each fixture array contains one child");
    }
    assert_eq!(value, Value::Null);
}
