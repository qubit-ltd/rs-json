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
use qubit_json::tree::JsonBudgetRejection;
use qubit_json::tree::JsonTreeContext;
use qubit_json::tree::JsonTreeControl;
use qubit_json::tree::JsonTreeMutVisitor;
use qubit_json::tree::JsonTreeProcessor;
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
    let structure = StructureLimits::empty()
        .with_depth_limit(ResourceLimit::new(JsonResource::Depth, GENEROUS_LIMIT))
        .with_nodes_limit(ResourceLimit::new(JsonResource::Nodes, GENEROUS_LIMIT))
        .with_sequence_items_limit(ResourceLimit::new(
            JsonResource::SequenceItems,
            GENEROUS_LIMIT,
        ))
        .with_map_entries_limit(ResourceLimit::new(JsonResource::MapEntries, GENEROUS_LIMIT))
        .with_key_bytes_limit(ResourceLimit::new(JsonResource::KeyBytes, GENEROUS_LIMIT));
    JsonValueBudget::new(
        JsonValueLimits::empty()
            .with_structure_limits(structure)
            .with_payload_bytes_limit(ResourceLimit::new(
                JsonResource::PayloadBytes,
                GENEROUS_LIMIT,
            )),
    )
}

/// Ensures a restored tree remains a valid JSON value after a traversal path.
fn assert_serializable(value: &Value) {
    let encoded = serde_json::to_vec(value)
        .expect("tree restoration must preserve a serializable JSON value");
    let decoded =
        serde_json::from_slice::<Value>(&encoded).expect("restored tree must remain valid JSON");
    assert_eq!(decoded, *value);
}

/// Mutates object nodes in the same shape as a redaction visitor.
struct SuccessVisitor;

impl JsonTreeMutVisitor<JsonResource, usize> for SuccessVisitor {
    type Error = std::convert::Infallible;

    /// Removes secret fields and descends into every admitted container.
    fn visit(
        &mut self,
        value: &mut Value,
        _context: JsonTreeContext<'_>,
    ) -> Result<JsonTreeControl, Self::Error> {
        if let Value::Object(entries) = value {
            entries.remove("secret");
        }
        Ok(JsonTreeControl::Descend)
    }
}

/// Replaces budget-rejected nodes and continues with the rest of the tree.
struct RejectingVisitor;

impl JsonTreeMutVisitor<JsonResource, usize> for RejectingVisitor {
    type Error = std::convert::Infallible;

    /// Descends through admitted nodes.
    fn visit(
        &mut self,
        _value: &mut Value,
        _context: JsonTreeContext<'_>,
    ) -> Result<JsonTreeControl, Self::Error> {
        Ok(JsonTreeControl::Descend)
    }

    /// Replaces a rejected subtree so restoration must retain a valid root.
    fn reject_budget(
        &mut self,
        value: &mut Value,
        _context: JsonTreeContext<'_>,
        _error: &qubit_budget::MeasuredBudgetError<JsonResource, usize>,
    ) -> Result<JsonBudgetRejection, Self::Error> {
        *value = Value::String("[redacted]".to_owned());
        Ok(JsonBudgetRejection::SkipSubtree)
    }
}

/// Returns a visitor error after a byte-selected number of callbacks.
struct ErrorVisitor {
    stop_after: usize,
    calls: usize,
}

impl JsonTreeMutVisitor<JsonResource, usize> for ErrorVisitor {
    type Error = &'static str;

    /// Mutates objects and then fails at the selected callback.
    fn visit(
        &mut self,
        value: &mut Value,
        _context: JsonTreeContext<'_>,
    ) -> Result<JsonTreeControl, Self::Error> {
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
impl JsonTreeMutVisitor<JsonResource, usize> for PanicVisitor {
    type Error = std::convert::Infallible;

    /// Mutates objects and then deliberately panics at the selected callback.
    fn visit(
        &mut self,
        value: &mut Value,
        _context: JsonTreeContext<'_>,
    ) -> Result<JsonTreeControl, Self::Error> {
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
    let mut success_budget = generous_budget();
    JsonTreeProcessor::new(&mut success_budget)
        .process_mut(&mut success_value, &mut SuccessVisitor)
        .expect("generous success traversal must complete");
    assert_serializable(&success_value);

    let node_limit = 1 + usize::from(data.first().copied().unwrap_or(0)) % 32;
    let structure = StructureLimits::empty()
        .with_nodes_limit(ResourceLimit::new(JsonResource::Nodes, node_limit));
    let mut rejected_value = original.clone();
    let mut rejected_budget =
        JsonValueBudget::new(JsonValueLimits::empty().with_structure_limits(structure));
    JsonTreeProcessor::new(&mut rejected_budget)
        .process_mut(&mut rejected_value, &mut RejectingVisitor)
        .expect("rejecting visitor must handle every budget rejection");
    assert_serializable(&rejected_value);
    assert!(rejected_budget.structure_budget().used_nodes() <= node_limit);

    let stop_after = 1 + usize::from(data.get(1).copied().unwrap_or(0)) % 16;
    let mut error_value = original.clone();
    let mut error_budget = generous_budget();
    let error = JsonTreeProcessor::new(&mut error_budget).process_mut(
        &mut error_value,
        &mut ErrorVisitor {
            stop_after,
            calls: 0,
        },
    );
    assert!(matches!(
        error,
        Err(qubit_json::tree::JsonTreeProcessError::Visitor(
            "fuzz visitor error"
        ))
    ));
    assert_serializable(&error_value);

    let panic_after = 1 + usize::from(data.get(2).copied().unwrap_or(0)) % 16;
    let mut recovery_value = original;
    #[cfg(all(not(fuzzing), panic = "unwind"))]
    {
        let mut panic_budget = generous_budget();
        let panic_result = catch_unwind(AssertUnwindSafe(|| {
            let _ = JsonTreeProcessor::new(&mut panic_budget).process_mut(
                &mut recovery_value,
                &mut PanicVisitor {
                    panic_after,
                    calls: 0,
                },
            );
        }));
        assert!(panic_result.is_err());
    }
    #[cfg(fuzzing)]
    {
        // cargo-fuzz uses panic=abort, so exercise the same restoration
        // boundary through a visitor error instead of terminating the fuzzer.
        let mut recovery_budget = generous_budget();
        let recovery_error = JsonTreeProcessor::new(&mut recovery_budget).process_mut(
            &mut recovery_value,
            &mut ErrorVisitor {
                stop_after: panic_after,
                calls: 0,
            },
        );
        assert!(recovery_error.is_err());
    }
    assert_serializable(&recovery_value);
});
