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
use qubit_json::value::traverse::JsonTreeBudgetRejection;
use qubit_json::value::traverse::JsonTreeContext;
use qubit_json::value::traverse::JsonTreeControl;
use qubit_json::value::traverse::JsonTreeMutVisitor;
use qubit_json::value::traverse::JsonTreeMutator;
use qubit_json::value::traverse::JsonTreeProcessError;
use qubit_json::value::traverse::JsonTreeReader;
use serde_json::Value;
use serde_json::json;
use serde_json::to_string;

struct FailingVisitor {
    calls: usize,
}

impl JsonTreeMutVisitor<JsonResource, usize> for FailingVisitor {
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

/// Replaces the first admitted array element before a later node is rejected.
struct FirstChildReplacingVisitor;

impl JsonTreeMutVisitor<JsonResource, usize> for FirstChildReplacingVisitor {
    type Error = std::convert::Infallible;

    fn visit(&mut self, value: &mut Value, _context: JsonTreeContext<'_>) -> Result<JsonTreeControl, Self::Error> {
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

    fn visit(&mut self, _value: &mut Value, _context: JsonTreeContext<'_>) -> Result<JsonTreeControl, Self::Error> {
        Ok(JsonTreeControl::Descend)
    }

    fn reject_budget(
        &mut self,
        value: &mut Value,
        _context: JsonTreeContext<'_>,
        _error: &MeasuredBudgetError<JsonResource, usize>,
    ) -> Result<JsonTreeBudgetRejection, Self::Error> {
        *value = json!("[redacted]");
        Ok(JsonTreeBudgetRejection::SkipSubtree)
    }
}

/// Leaves budget-rejected containers in place while requesting subtree
/// skipping.
struct RetainingRejectedContainerVisitor {
    visits: usize,
    rejections: usize,
}

impl JsonTreeMutVisitor<JsonResource, usize> for RetainingRejectedContainerVisitor {
    type Error = std::convert::Infallible;

    fn visit(&mut self, _value: &mut Value, _context: JsonTreeContext<'_>) -> Result<JsonTreeControl, Self::Error> {
        self.visits += 1;
        Ok(JsonTreeControl::Descend)
    }

    fn reject_budget(
        &mut self,
        _value: &mut Value,
        _context: JsonTreeContext<'_>,
        _error: &MeasuredBudgetError<JsonResource, usize>,
    ) -> Result<JsonTreeBudgetRejection, Self::Error> {
        self.rejections += 1;
        Ok(JsonTreeBudgetRejection::SkipSubtree)
    }
}

/// Panics after mutating a selected node.
struct PanickingVisitor {
    calls: usize,
    panic_after: usize,
}

impl JsonTreeMutVisitor<JsonResource, usize> for PanickingVisitor {
    type Error = std::convert::Infallible;

    /// Mutates the current object before deliberately panicking.
    fn visit(&mut self, value: &mut Value, _context: JsonTreeContext<'_>) -> Result<JsonTreeControl, Self::Error> {
        self.calls += 1;
        if let Value::Object(entries) = value {
            entries.insert("visited".to_owned(), json!(true));
        }
        assert!(self.calls < self.panic_after, "visitor panic regression");
        Ok(JsonTreeControl::Descend)
    }
}

/// Verifies that a rejection can replace exactly the rejected subtree.
#[test]
fn test_process_mut_skips_rejected_subtree_after_replacement() {
    let limits = JsonValueLimits::<JsonResource, usize>::builder()
        .structure_limits(StructureLimits::builder().nodes_limit(ResourceLimit::new(JsonResource::Nodes, 2_usize)))
        .build();
    let mut budget = JsonValueBudget::new(limits);
    let mut transaction = budget.transaction();
    let mut value = json!({"first": true, "second": {"nested": false}});

    JsonTreeMutator::new(&mut transaction)
        .process(&mut value, &mut ReplacingVisitor)
        .expect("visitor handles the resource rejection");

    assert_eq!(value, json!({"first": true, "second": "[redacted]"}));
}

/// Verifies that SkipSubtree also skips a retained container and all
/// descendants.
#[test]
fn test_process_mut_skips_rejected_container_subtree() {
    let limits = JsonValueLimits::<JsonResource, usize>::builder()
        .structure_limits(StructureLimits::builder().nodes_limit(ResourceLimit::new(JsonResource::Nodes, 2_usize)))
        .build();
    let mut budget = JsonValueBudget::new(limits);
    let mut transaction = budget.transaction();
    let mut value = json!({
        "first": true,
        "second": {"nested": {"leaf": false}},
    });
    let mut visitor = RetainingRejectedContainerVisitor {
        visits: 0,
        rejections: 0,
    };

    JsonTreeMutator::new(&mut transaction)
        .process(&mut value, &mut visitor)
        .expect("visitor handles the resource rejection");

    assert_eq!(visitor.visits, 2);
    assert_eq!(visitor.rejections, 1);
    assert_eq!(value["second"], json!({"nested": {"leaf": false}}));
}

#[test]
fn test_process_mut_preserves_mutations_when_visitor_fails() {
    let limits = JsonValueLimits::<JsonResource, usize>::builder()
        .structure_limits(StructureLimits::builder().nodes_limit(ResourceLimit::new(JsonResource::Nodes, 4_usize)))
        .payload_bytes_limit(ResourceLimit::new(JsonResource::PayloadBytes, 32_usize))
        .build();
    let mut budget = JsonValueBudget::new(limits);
    let mut transaction = budget.transaction();
    let mut value = json!({"original": true});

    let error = JsonTreeMutator::new(&mut transaction)
        .process(&mut value, &mut FailingVisitor { calls: 0 })
        .expect_err("the visitor deliberately fails");

    assert!(matches!(error, JsonTreeProcessError::Visitor("stop")));
    assert_eq!(value, json!({"changed": "partially changed"}));
    assert_eq!(
        to_string(&value).expect("partially mutated value serializes"),
        r#"{"changed":"partially changed"}"#,
    );
    assert_eq!(transaction.used_nodes(), Some(2));
    assert_eq!(transaction.used_payload_bytes(), Some(7));
}

/// Verifies that budget rejection retains earlier mutation and accounting.
#[test]
fn test_process_mut_preserves_partial_mutation_and_budget_on_rejection() {
    let limits = JsonValueLimits::<JsonResource, usize>::builder()
        .structure_limits(StructureLimits::builder().nodes_limit(ResourceLimit::new(JsonResource::Nodes, 2_usize)))
        .payload_bytes_limit(ResourceLimit::new(JsonResource::PayloadBytes, 8_usize))
        .build();
    let mut budget = JsonValueBudget::new(limits);
    let mut transaction = budget.transaction();
    let mut value = json!([0, 1]);

    let error = JsonTreeMutator::new(&mut transaction)
        .process(&mut value, &mut FirstChildReplacingVisitor)
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
    assert_eq!(transaction.used_nodes(), Some(2));
    assert_eq!(transaction.used_payload_bytes(), Some(1));
}

/// Verifies read-only and mutable traversal stage identical resource usage.
#[test]
fn test_reader_and_mutator_measure_the_same_json_tree() {
    let limits = JsonValueLimits::<JsonResource, usize>::builder()
        .max_nodes(8)
        .max_payload_bytes(128)
        .build();
    let value = json!({"name": "qubit", "values": [1.25, true, null]});

    let mut read_budget = limits.budget();
    let mut read_transaction = read_budget.transaction();
    JsonTreeReader::new(&mut read_transaction)
        .account(&value)
        .expect("reader should admit the fixture");

    let mut mutable_value = value.clone();
    let mut mut_budget = limits.budget();
    let mut mut_transaction = mut_budget.transaction();
    JsonTreeMutator::new(&mut mut_transaction)
        .process(&mut mutable_value, &mut ReplacingVisitor)
        .expect("mutator should admit the fixture");

    assert_eq!(mut_transaction.used_nodes(), read_transaction.used_nodes());
    assert_eq!(
        mut_transaction.used_payload_bytes(),
        read_transaction.used_payload_bytes(),
    );
}

/// Verifies that a panic leaves the mutable root reassembled and serializable.
#[test]
fn test_process_mut_restores_root_after_visitor_panic() {
    let mut budget = JsonValueBudget::new(JsonValueLimits::<JsonResource, usize>::builder().build());
    let mut transaction = budget.transaction();
    let mut value = json!({"first": [1, 2], "second": true});

    let result = catch_unwind(AssertUnwindSafe(|| {
        let _ = JsonTreeMutator::new(&mut transaction).process(
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
    assert_eq!(
        to_string(&value).expect("panic-restored value serializes"),
        r#"{"first":[1,2],"second":true,"visited":true}"#,
    );
}
use std::panic::AssertUnwindSafe;
use std::panic::catch_unwind;
