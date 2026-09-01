// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
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

struct ExpandingVisitor;

impl JsonTreeMutVisitor for ExpandingVisitor {
    type Error = std::convert::Infallible;

    fn visit(
        &mut self,
        value: &mut Value,
        _context: JsonTreeContext<'_>,
    ) -> Result<JsonTreeControl, Self::Error> {
        *value = json!([null]);
        Ok(JsonTreeControl::SkipSubtree)
    }
}

/// Verifies output budget failures identify the post-mutation phase.
///
/// # Panics
///
/// Panics when an expanded result bypasses output accounting or loses its
/// failure phase.
#[test]
fn test_output_budget_error_identifies_mutation_phase() {
    let mut input_budget = JsonValueBudget::new(JsonValueLimits::<JsonResource, usize>::new());
    let mut output_budget = JsonValueBudget::new(
        JsonValueLimits::<JsonResource, usize>::builder()
            .max_nodes(1)
            .build(),
    );
    let mut input = input_budget.transaction();
    let mut output = output_budget.transaction();
    let mut value = Value::Null;

    let error = JsonTreeMutator::new(&mut input, &mut output)
        .process(&mut value, &mut ExpandingVisitor)
        .expect_err("the expanded result must exceed its output node budget");

    assert!(matches!(
        error,
        JsonTreeMutateError::OutputBudget(error)
            if error.resource() == &JsonResource::Nodes
    ));
    assert_eq!(value, json!([null]));
    assert_eq!(input.used_nodes(), None);
    assert_eq!(output.used_nodes(), Some(1));
}
