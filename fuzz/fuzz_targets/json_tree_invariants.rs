// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Fuzzes mutable JSON tree restoration and visitor-control invariants.

#![no_main]

use std::convert::Infallible;
#[cfg(all(not(fuzzing), panic = "unwind"))]
use std::panic::AssertUnwindSafe;
#[cfg(all(not(fuzzing), panic = "unwind"))]
use std::panic::catch_unwind;

use libfuzzer_sys::fuzz_target;
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
use qubit_json_fuzz::input_limit::bounded_input;
use qubit_json_fuzz::json_reference::parse_literal_value;
use qubit_json_fuzz::json_reference::roundtrip_literal_value;
use serde_json::Value;
use serde_json::json;

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

fuzz_target!(|data: &[u8]| {
    let Some(data) = bounded_input(data) else {
        return;
    };
    let (control, payload) = data
        .split_first()
        .map_or((0, data), |(control, payload)| (*control, payload));
    let original = parse_literal_value(payload).unwrap_or_else(|_| make_tree(payload));

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
    if node_count(&limited_value) > 1 {
        assert!(matches!(
            limited_error,
            Err(JsonTreeMutateError::OutputBudget(error)) if error.resource() == &JsonResource::Nodes
        ));
    } else {
        assert!(
            limited_error.is_ok(),
            "a one-node result must fit the one-node output budget"
        );
    }

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
    let error = {
        let mut input_transaction = error_input_budget.transaction();
        let mut output_transaction = error_output_budget.transaction();
        JsonTreeMutator::new(&mut input_transaction, &mut output_transaction)
            .process(&mut error_value, &mut ErrorVisitor { stop_after, calls: 0 })
    };
    assert!(matches!(
        error,
        Ok(()) | Err(JsonTreeMutateError::Visitor("fuzz visitor error"))
    ));
    assert_serializable(&error_value);

    #[cfg(all(not(fuzzing), panic = "unwind"))]
    let panic_after = 1;
    let mut recovery_value = original;
    #[cfg(all(not(fuzzing), panic = "unwind"))]
    {
        let mut panic_input_budget = generous_budget();
        let mut panic_output_budget = generous_budget();
        let mut input_transaction = panic_input_budget.transaction();
        let mut output_transaction = panic_output_budget.transaction();
        // libfuzzer-sys aborts from its panic hook before catch_unwind can recover.
        let panic_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let panic_result = catch_unwind(AssertUnwindSafe(|| {
            let _ = JsonTreeMutator::new(&mut input_transaction, &mut output_transaction)
                .process(&mut recovery_value, &mut PanicVisitor { panic_after, calls: 0 });
        }));
        std::panic::set_hook(panic_hook);
        assert!(panic_result.is_err());
    }
    #[cfg(fuzzing)]
    {
        // cargo-fuzz uses panic=abort, so exercise the same restoration
        // boundary through a visitor error instead of terminating the fuzzer.
        let mut recovery_input_budget = generous_budget();
        let mut recovery_output_budget = generous_budget();
        let recovery_error = {
            let mut input_transaction = recovery_input_budget.transaction();
            let mut output_transaction = recovery_output_budget.transaction();
            JsonTreeMutator::new(&mut input_transaction, &mut output_transaction).process(
                &mut recovery_value,
                &mut ErrorVisitor {
                    // A fixed first callback guarantees that the abort-safe
                    // surrogate reaches the same restoration boundary on
                    // every fuzz input.
                    stop_after: 1,
                    calls: 0,
                },
            )
        };
        assert!(matches!(
            recovery_error,
            Err(JsonTreeMutateError::Visitor("fuzz visitor error"))
        ));
    }
    assert_serializable(&recovery_value);
});
