// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
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

/// Replaces budget-rejected values and descends into all admitted values.
struct ReplacingVisitor;

impl JsonTreeMutVisitor<JsonResource, usize> for ReplacingVisitor {
    type Error = std::convert::Infallible;

    fn visit(
        &mut self,
        _value: &mut Value,
        _context: JsonTreeContext<'_>,
    ) -> Result<JsonTreeControl, Self::Error> {
        Ok(JsonTreeControl::Descend)
    }

    fn reject_budget(
        &mut self,
        value: &mut Value,
        _context: JsonTreeContext<'_>,
        _error: &qubit_budget::MeasuredBudgetError<JsonResource, usize>,
    ) -> Result<JsonBudgetRejection, Self::Error> {
        *value = json!("[redacted]");
        Ok(JsonBudgetRejection::SkipSubtree)
    }
}

/// Verifies that a rejection can replace exactly the rejected subtree.
#[test]
fn test_process_mut_skips_rejected_subtree_after_replacement() {
    let limits = JsonValueLimits::empty().with_structure_limits(
        StructureLimits::empty().with_nodes_limit(ResourceLimit::new(JsonResource::Nodes, 2_usize)),
    );
    let mut budget = JsonValueBudget::new(limits);
    let mut value = json!({"first": true, "second": {"nested": false}});

    JsonTreeProcessor::new(&mut budget)
        .process_mut(&mut value, &mut ReplacingVisitor)
        .expect("visitor handles the resource rejection");

    assert_eq!(value, json!({"first": true, "second": "[redacted]"}));
}
