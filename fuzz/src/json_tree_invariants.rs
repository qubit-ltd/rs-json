// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Fuzzes mutable JSON tree restoration and visitor-control invariants.

use std::convert::Infallible;
#[cfg(all(not(fuzzing), panic = "unwind"))]
use std::panic::AssertUnwindSafe;
#[cfg(all(not(fuzzing), panic = "unwind"))]
use std::panic::catch_unwind;

use qubit_budget::ResourceLimit;
use qubit_budget::StructureLimits;
use qubit_budget::json::JsonResource;
use qubit_budget::json::JsonValueBudget;
use qubit_budget::json::JsonValueLimits;
use qubit_json::value::traverse::JsonTreeContext;
use qubit_json::value::traverse::JsonTreeControl;
use qubit_json::value::traverse::JsonTreeMutVisitor;
use qubit_json::value::traverse::JsonTreeMutateError;
use qubit_json::value::traverse::JsonTreeMutator;
use serde_json::Value;
use serde_json::json;

use crate::input_limit::bounded_input;
use crate::json_reference::parse_literal_value;
use crate::json_reference::roundtrip_literal_value;

const MAX_TREE_DEPTH: usize = 64;
const GENEROUS_LIMIT: usize = 1_000_000;

/// Builds a bounded tree with wide and deep shapes from arbitrary bytes.
fn make_tree(data: &[u8]) -> Value {
    let mut items = Vec::new();
    for chunk in data.chunks(4).take(128) {
        let number = chunk.iter().fold(0_u64, |value, byte| {
            value.saturating_mul(257).saturating_add(u64::from(*byte))
        });
        items.push(json!({
            "keep": number,
            "secret": "TOP_SECRET",
            "nested": [number, number % 7],
        }));
    }
    let mut value = json!({"keep": true, "secret": "TOP_SECRET", "items": items});
    for level in data.iter().take(MAX_TREE_DEPTH).rev() {
        value = json!({
            "level": level,
            "secret": "TOP_SECRET",
            "child": value,
        });
    }
    value
}

/// Creates a budget with generous limits for visitor-error and panic paths.
#[must_use]
fn generous_budget() -> JsonValueBudget<JsonResource, usize> {
    let structure = StructureLimits::builder()
        .depth_limit(ResourceLimit::new(JsonResource::Depth, GENEROUS_LIMIT))
        .nodes_limit(ResourceLimit::new(JsonResource::Nodes, GENEROUS_LIMIT))
        .sequence_items_limit(ResourceLimit::new(JsonResource::SequenceItems, GENEROUS_LIMIT))
        .map_entries_limit(ResourceLimit::new(JsonResource::MapEntries, GENEROUS_LIMIT))
        .key_bytes_limit(ResourceLimit::new(JsonResource::KeyBytes, GENEROUS_LIMIT));
    JsonValueBudget::new(
        JsonValueLimits::<JsonResource, usize>::builder()
            .structure_limits(structure)
            .payload_bytes_limit(ResourceLimit::new(JsonResource::PayloadBytes, GENEROUS_LIMIT))
            .build(),
    )
}

/// Ensures a restored tree remains a valid JSON value after a traversal path.
fn assert_serializable(value: &Value) {
    let decoded = roundtrip_literal_value(value).expect("restored tree must remain valid literal JSON");
    assert_eq!(decoded, *value);
}

/// Verifies a successful redaction traversal removed every secret field.
fn assert_no_secret_fields(value: &Value) {
    let mut pending = vec![value];
    while let Some(value) = pending.pop() {
        match value {
            Value::Array(values) => pending.extend(values),
            Value::Object(entries) => {
                assert!(!entries.contains_key("secret"));
                pending.extend(entries.values());
            }
            _ => {}
        }
    }
}

/// Removes secret fields and selects whether traversal enters containers.
struct ControlVisitor {
    skip_subtree: bool,
}

impl JsonTreeMutVisitor for ControlVisitor {
    type Error = Infallible;

    /// Applies redaction and returns the fuzz-selected subtree control.
    fn visit(&mut self, value: &mut Value, _context: JsonTreeContext<'_>) -> Result<JsonTreeControl, Self::Error> {
        if let Value::Object(entries) = value {
            entries.remove("secret");
        }
        Ok(if self.skip_subtree {
            JsonTreeControl::SkipSubtree
        } else {
            JsonTreeControl::Descend
        })
    }
}

/// Counts every JSON value and container in the completed tree.
fn node_count(value: &Value) -> usize {
    let children = match value {
        Value::Array(values) => values.iter().map(node_count).sum(),
        Value::Object(entries) => entries.values().map(node_count).sum(),
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => 0,
    };
    1 + children
}

/// Returns a visitor error after a byte-selected number of callbacks.
struct ErrorVisitor {
    stop_after: usize,
    calls: usize,
}

impl JsonTreeMutVisitor for ErrorVisitor {
    type Error = &'static str;

    /// Mutates objects and then fails at the selected callback.
    fn visit(&mut self, value: &mut Value, _context: JsonTreeContext<'_>) -> Result<JsonTreeControl, Self::Error> {
        self.calls = self.calls.saturating_add(1);
        if let Value::Object(entries) = value {
            entries.remove("secret");
        }
        if self.calls >= self.stop_after {
            Err("fuzz visitor error")
        } else {
            Ok(JsonTreeControl::Descend)
        }
    }
}

/// Panics after a byte-selected number of callbacks when unwinding is enabled.
#[cfg(all(not(fuzzing), panic = "unwind"))]
struct PanicVisitor {
    panic_after: usize,
    calls: usize,
}

#[cfg(all(not(fuzzing), panic = "unwind"))]
impl JsonTreeMutVisitor for PanicVisitor {
    type Error = Infallible;

    /// Mutates objects and then deliberately panics at the selected callback.
    fn visit(&mut self, value: &mut Value, _context: JsonTreeContext<'_>) -> Result<JsonTreeControl, Self::Error> {
        self.calls = self.calls.saturating_add(1);
        if let Value::Object(entries) = value {
            entries.remove("secret");
        }
        assert!(self.calls < self.panic_after, "fuzz visitor panic");
        Ok(JsonTreeControl::Descend)
    }
}

/// Runs all bounded mutable-tree restoration invariants for one fuzz input.
pub fn assert_json_tree_invariants(data: &[u8]) {
    let Some(data) = bounded_input(data) else {
        return;
    };
    let (control, payload): (u8, &[u8]) = match data.split_first() {
        Some((control, payload)) => (*control, payload),
        None => (0, data),
    };
    let original = parse_literal_value(payload).unwrap_or_else(|_| make_tree(payload));

    let (success_expected, _, _) = reference_redaction(&original, control & 1 != 0, None);
    let mut success_value = original.clone();
    let mut success_input_budget = generous_budget();
    let mut success_output_budget = generous_budget();
    {
        let mut input_transaction = success_input_budget.transaction();
        let mut output_transaction = success_output_budget.transaction();
        JsonTreeMutator::new(&mut input_transaction, &mut output_transaction)
            .process(
                &mut success_value,
                &mut ControlVisitor {
                    skip_subtree: control & 1 != 0,
                },
            )
            .expect("generous success traversal must complete");
        assert_eq!(output_transaction.used_nodes(), Some(node_count(&success_value)));
        input_transaction.commit().expect("input transaction commits");
        output_transaction.commit().expect("output transaction commits");
    }
    assert_eq!(success_value, success_expected, "successful traversal content");
    assert_serializable(&success_value);
    if control & 1 == 0 {
        assert_no_secret_fields(&success_value);
    }

    let mut limited_value = original.clone();
    let mut limited_input_budget = generous_budget();
    let mut limited_output_budget = JsonValueBudget::new(
        JsonValueLimits::<JsonResource, usize>::builder()
            .structure_limits(StructureLimits::builder().nodes_limit(ResourceLimit::new(JsonResource::Nodes, 1)))
            .build(),
    );
    let (limited_expected, _, _) = reference_redaction(&original, control & 1 != 0, None);
    let expected_fits_one_node = node_count(&limited_expected) <= 1;
    let limited_error = {
        let mut input_transaction = limited_input_budget.transaction();
        let mut output_transaction = limited_output_budget.transaction();
        JsonTreeMutator::new(&mut input_transaction, &mut output_transaction).process(
            &mut limited_value,
            &mut ControlVisitor {
                skip_subtree: control & 1 != 0,
            },
        )
    };
    if !expected_fits_one_node {
        assert!(matches!(
            limited_error,
            Err(JsonTreeMutateError::OutputBudget(error))
                if error.resource() == &JsonResource::Nodes
        ));
    } else {
        assert!(
            limited_error.is_ok(),
            "a one-node result must fit the one-node output budget"
        );
    }
    assert_eq!(limited_value, limited_expected, "output-budget traversal content");

    // Every JSON value contains a root node, so a zero-node limit always
    // rejects input before the visitor can mutate the tree.
    let structure = StructureLimits::builder().nodes_limit(ResourceLimit::new(JsonResource::Nodes, 0));
    let mut rejected_value = original.clone();
    let mut rejected_input_budget = JsonValueBudget::new(
        JsonValueLimits::<JsonResource, usize>::builder()
            .structure_limits(structure)
            .build(),
    );
    let mut rejected_output_budget = generous_budget();
    {
        let mut input_transaction = rejected_input_budget.transaction();
        let mut output_transaction = rejected_output_budget.transaction();
        let result = JsonTreeMutator::new(&mut input_transaction, &mut output_transaction).process(
            &mut rejected_value,
            &mut ControlVisitor {
                skip_subtree: control & 1 != 0,
            },
        );
        assert!(matches!(result, Err(JsonTreeMutateError::InputBudget(_))));
    }
    assert_eq!(rejected_value, original);
    assert_serializable(&rejected_value);

    let stop_after = 1 + usize::from(data.get(1).copied().unwrap_or(0)) % 16;
    let mut error_value = original.clone();
    let mut error_input_budget = generous_budget();
    let mut error_output_budget = generous_budget();
    let (error_expected, expected_calls, stopped) = reference_redaction(&original, false, Some(stop_after));
    let mut error_visitor = ErrorVisitor { stop_after, calls: 0 };
    let error = {
        let mut input_transaction = error_input_budget.transaction();
        let mut output_transaction = error_output_budget.transaction();
        JsonTreeMutator::new(&mut input_transaction, &mut output_transaction)
            .process(&mut error_value, &mut error_visitor)
    };
    assert_eq!(error_visitor.calls, expected_calls);
    assert_eq!(error_value, error_expected, "visitor-error traversal content");
    assert_eq!(
        matches!(error, Err(JsonTreeMutateError::Visitor("fuzz visitor error"))),
        stopped
    );
    assert_serializable(&error_value);

    #[cfg(all(not(fuzzing), panic = "unwind"))]
    let panic_after = 1;
    let mut recovery_value = original.clone();
    #[cfg(all(not(fuzzing), panic = "unwind"))]
    {
        let mut panic_input_budget = generous_budget();
        let mut panic_output_budget = generous_budget();
        let mut input_transaction = panic_input_budget.transaction();
        let mut output_transaction = panic_output_budget.transaction();
        let (recovery_expected, recovery_calls, _) = reference_redaction(&original, false, Some(panic_after));
        let mut panic_visitor = PanicVisitor { panic_after, calls: 0 };
        let panic_result = catch_unwind(AssertUnwindSafe(|| {
            let _ = JsonTreeMutator::new(&mut input_transaction, &mut output_transaction)
                .process(&mut recovery_value, &mut panic_visitor);
        }));
        assert!(panic_result.is_err());
        assert_eq!(panic_visitor.calls, recovery_calls);
        assert_eq!(recovery_value, recovery_expected, "panic recovery traversal content");
    }
    #[cfg(fuzzing)]
    {
        // cargo-fuzz uses panic=abort, so exercise the same restoration
        // boundary through a visitor error instead of terminating the fuzzer.
        let mut recovery_input_budget = generous_budget();
        let mut recovery_output_budget = generous_budget();
        let (recovery_expected, recovery_calls, _) = reference_redaction(&original, false, Some(1));
        let mut recovery_visitor = ErrorVisitor {
            stop_after: 1,
            calls: 0,
        };
        let recovery_error = {
            let mut input_transaction = recovery_input_budget.transaction();
            let mut output_transaction = recovery_output_budget.transaction();
            JsonTreeMutator::new(&mut input_transaction, &mut output_transaction)
                .process(&mut recovery_value, &mut recovery_visitor)
        };
        assert!(matches!(
            recovery_error,
            Err(JsonTreeMutateError::Visitor("fuzz visitor error"))
        ));
        assert_eq!(recovery_visitor.calls, recovery_calls);
        assert_eq!(recovery_value, recovery_expected, "recovery surrogate content");
    }
    assert_serializable(&recovery_value);
}

/// Applies the expected redaction using ordinary recursive Value traversal.
fn reference_redaction(original: &Value, skip_subtree: bool, stop_after: Option<usize>) -> (Value, usize, bool) {
    fn visit(value: &mut Value, skip: bool, stop: Option<usize>, calls: &mut usize) -> Result<(), ()> {
        *calls += 1;
        if let Value::Object(entries) = value {
            entries.remove("secret");
        }
        if stop == Some(*calls) {
            return Err(());
        }
        if !skip {
            match value {
                Value::Array(values) => {
                    for child in values {
                        visit(child, skip, stop, calls)?;
                    }
                }
                Value::Object(entries) => {
                    for child in entries.values_mut() {
                        visit(child, skip, stop, calls)?;
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }
    let mut expected = original.clone();
    let mut calls = 0;
    let stopped = visit(&mut expected, skip_subtree, stop_after, &mut calls).is_err();
    (expected, calls, stopped)
}

#[cfg(test)]
mod tests {
    use serde_json::Value;
    use serde_json::json;

    use super::reference_redaction;

    #[test]
    fn reference_redaction_matches_handwritten_partial_result() {
        let original = json!({
            "secret": "root",
            "a": {"secret": "child", "keep": [1, 2]},
            "z": 7,
        });
        let (expected, calls, stopped) = reference_redaction(&original, false, Some(1));
        assert_eq!(expected, json!({"a": {"secret": "child", "keep": [1, 2]}, "z": 7}));
        assert_eq!(calls, 1);
        assert!(stopped);
    }

    #[test]
    fn reference_redaction_covers_shapes_and_stop_positions() {
        for original in [
            Value::Null,
            json!([{"secret":"a", "keep":1}, [2, 3]]),
            json!({}),
            json!({"兄弟":{"secret":"x","$serde_json::private::RawValue":"y"}, "z":7}),
        ] {
            let (_, total, stopped) = reference_redaction(&original, false, None);
            assert!(!stopped);
            for stop_after in [1, total / 2 + 1, total, total + 1] {
                let (_, calls, stopped) = reference_redaction(&original, false, Some(stop_after));
                assert_eq!(calls, stop_after.min(total));
                assert_eq!(stopped, stop_after <= total);
            }
            let (_, calls, stopped) = reference_redaction(&original, true, None);
            assert_eq!(calls, 1);
            assert!(!stopped);
            if let Value::Object(entries) = original {
                assert!(!entries.contains_key("secret"));
            }
        }
    }

    #[test]
    fn complete_value_oracle_detects_representative_corruption() {
        let original = json!({"secret":"root", "a":{"secret":"child","keep":[1,2]}, "z":7});
        let (expected, _, _) = reference_redaction(&original, false, None);
        for corrupted in [
            json!({"a":{"secret":"child","keep":[1,2]}, "z":7}),
            json!({"a":{"keep":[1]}, "z":7}),
            json!({"a":null, "z":7}),
            json!({"a":{"keep":[1,2]}}),
        ] {
            assert!(std::panic::catch_unwind(|| assert_eq!(corrupted, expected)).is_err());
        }
    }
}
