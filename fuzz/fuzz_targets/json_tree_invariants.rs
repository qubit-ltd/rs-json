// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Fuzzes mutable JSON tree restoration and visitor-control invariants.

#![no_main]

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
use serde_json::Value;
use serde_json::json;

const MAX_INPUT_LEN: usize = 4 * 1024;
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
    let encoded = serde_json::to_vec(value).expect("tree restoration must preserve a serializable JSON value");
    let decoded = serde_json::from_slice::<Value>(&encoded).expect("restored tree must remain valid JSON");
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

/// Mutates object nodes in the same shape as a redaction visitor.
struct SuccessVisitor;

impl JsonTreeMutVisitor for SuccessVisitor {
    type Error = std::convert::Infallible;

    /// Removes secret fields and descends into every admitted container.
    fn visit(&mut self, value: &mut Value, _context: JsonTreeContext<'_>) -> Result<JsonTreeControl, Self::Error> {
        if let Value::Object(entries) = value {
            entries.remove("secret");
        }
        Ok(JsonTreeControl::Descend)
    }
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
    type Error = std::convert::Infallible;

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
    let input = &data[..data.len().min(MAX_INPUT_LEN)];
    let original = make_tree(input);

    let mut success_value = original.clone();
    let mut success_input_budget = generous_budget();
    let mut success_output_budget = generous_budget();
    {
        let mut input_transaction = success_input_budget.transaction();
        let mut output_transaction = success_output_budget.transaction();
        JsonTreeMutator::new(&mut input_transaction, &mut output_transaction)
            .process(&mut success_value, &mut SuccessVisitor)
            .expect("generous success traversal must complete");
        input_transaction.commit().expect("input transaction commits");
        output_transaction.commit().expect("output transaction commits");
    }
    assert_serializable(&success_value);
    assert_no_secret_fields(&success_value);

    // Even an empty fuzz input produces the root object, two scalar fields,
    // and the items array, so a maximum of three nodes always rejects input.
    let node_limit = 1 + usize::from(data.first().copied().unwrap_or(0)) % 3;
    let structure = StructureLimits::builder().nodes_limit(ResourceLimit::new(JsonResource::Nodes, node_limit));
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
        let result = JsonTreeMutator::new(&mut input_transaction, &mut output_transaction)
            .process(&mut rejected_value, &mut SuccessVisitor);
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
    let panic_after = 1 + usize::from(data.get(2).copied().unwrap_or(0)) % 16;
    let mut recovery_value = original;
    #[cfg(all(not(fuzzing), panic = "unwind"))]
    {
        let mut panic_input_budget = generous_budget();
        let mut panic_output_budget = generous_budget();
        let mut input_transaction = panic_input_budget.transaction();
        let mut output_transaction = panic_output_budget.transaction();
        let panic_result = catch_unwind(AssertUnwindSafe(|| {
            let _ = JsonTreeMutator::new(&mut input_transaction, &mut output_transaction)
                .process(&mut recovery_value, &mut PanicVisitor { panic_after, calls: 0 });
        }));
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
