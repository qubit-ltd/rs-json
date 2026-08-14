// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use qubit_budget::MeasuredBudgetError;
use qubit_budget::ResourceLimit;
use qubit_budget::StructureLimits;
use qubit_budget::json::JsonResource;
use qubit_budget::json::JsonValueBudget;
use qubit_budget::json::JsonValueLimits;
use qubit_json::tree::JsonBudgetRejection;
use qubit_json::tree::JsonTreeContext;
use qubit_json::tree::JsonTreeControl;
use qubit_json::tree::JsonTreeMutVisitor;
use qubit_json::tree::JsonTreeProcessError;
use qubit_json::tree::JsonTreeProcessor;
use serde_json::Value;
use serde_json::json;
use serde_json::to_string;

struct FailingVisitor {
    calls: usize,
}

impl JsonTreeMutVisitor<JsonResource, usize> for FailingVisitor {
    type Error = &'static str;

    fn visit(
        &mut self,
        value: &mut Value,
        _context: JsonTreeContext<'_>,
    ) -> Result<JsonTreeControl, Self::Error> {
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

/// Replaces the first admitted array element before a later node is rejected.
struct FirstChildReplacingVisitor;

impl JsonTreeMutVisitor<JsonResource, usize> for FirstChildReplacingVisitor {
    type Error = std::convert::Infallible;

    fn visit(
        &mut self,
        value: &mut Value,
        _context: JsonTreeContext<'_>,
    ) -> Result<JsonTreeControl, Self::Error> {
        match value {
            Value::Array(_) => Ok(JsonTreeControl::Descend),
            Value::Number(number) if number.as_i64() == Some(0) => {
                *value = json!("changed");
                Ok(JsonTreeControl::SkipSubtree)
            }
            _ => Ok(JsonTreeControl::SkipSubtree),
        }
    }
}

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
        _error: &MeasuredBudgetError<JsonResource, usize>,
    ) -> Result<JsonBudgetRejection, Self::Error> {
        *value = json!("[redacted]");
        Ok(JsonBudgetRejection::SkipSubtree)
    }
}

/// Verifies that a rejection can replace exactly the rejected subtree.
#[test]
fn test_process_mut_skips_rejected_subtree_after_replacement() {
    let limits = JsonValueLimits::empty()
        .with_structure_limits(StructureLimits::empty().with_nodes_limit(
            ResourceLimit::new(JsonResource::Nodes, 2_usize),
        ));
    let mut budget = JsonValueBudget::new(limits);
    let mut value = json!({"first": true, "second": {"nested": false}});

    JsonTreeProcessor::new(&mut budget)
        .process_mut(&mut value, &mut ReplacingVisitor)
        .expect("visitor handles the resource rejection");

    assert_eq!(value, json!({"first": true, "second": "[redacted]"}));
}

#[test]
fn test_process_mut_preserves_mutations_when_visitor_fails() {
    let limits =
        JsonValueLimits::empty()
            .with_structure_limits(StructureLimits::empty().with_nodes_limit(
                ResourceLimit::new(JsonResource::Nodes, 4_usize),
            ))
            .with_payload_bytes_limit(ResourceLimit::new(
                JsonResource::PayloadBytes,
                32_usize,
            ));
    let mut budget = JsonValueBudget::new(limits);
    let mut value = json!({"original": true});

    let error = JsonTreeProcessor::new(&mut budget)
        .process_mut(&mut value, &mut FailingVisitor { calls: 0 })
        .expect_err("the visitor deliberately fails");

    assert!(matches!(error, JsonTreeProcessError::Visitor("stop")));
    assert_eq!(value, json!({"changed": "partially changed"}));
    assert_eq!(
        to_string(&value).expect("partially mutated value serializes"),
        r#"{"changed":"partially changed"}"#,
    );
    assert_eq!(budget.structure_budget().used_nodes(), 2);
    assert_eq!(
        budget
            .payload_budget()
            .expect("payload budget is configured")
            .used(),
        7,
    );
}

/// Verifies that budget rejection retains earlier mutation and accounting.
#[test]
fn test_process_mut_preserves_partial_mutation_and_budget_on_rejection() {
    let limits =
        JsonValueLimits::empty()
            .with_structure_limits(StructureLimits::empty().with_nodes_limit(
                ResourceLimit::new(JsonResource::Nodes, 2_usize),
            ))
            .with_payload_bytes_limit(ResourceLimit::new(
                JsonResource::PayloadBytes,
                8_usize,
            ));
    let mut budget = JsonValueBudget::new(limits);
    let mut value = json!([0, 1]);

    let error = JsonTreeProcessor::new(&mut budget)
        .process_mut(&mut value, &mut FirstChildReplacingVisitor)
        .expect_err("the second child exceeds the node budget");

    assert!(matches!(
        &error,
        JsonTreeProcessError::Budget(error)
            if error.resource() == &JsonResource::Nodes
    ));
    assert_eq!(value, json!(["changed", 1]));
    assert_eq!(
        to_string(&value).expect("partially mutated value serializes"),
        r#"["changed",1]"#,
    );
    assert_eq!(budget.structure_budget().used_nodes(), 2);
    assert_eq!(
        budget
            .payload_budget()
            .expect("payload budget is configured")
            .used(),
        1,
    );
}
