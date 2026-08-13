// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use qubit_budget::ResourceLimit;
use qubit_budget::StructureLimits;
use qubit_json::BudgetedJsonValueSeed;
use qubit_json::JsonResource;
use qubit_json::JsonValueBudget;
use qubit_json::JsonValueLimits;
use serde::de::DeserializeSeed;
use serde_json::Deserializer;
use serde_json::json;

#[test]
fn budgeted_value_seed_rejects_decoded_nodes_incrementally() {
    let limits = JsonValueLimits::empty().with_structure_limits(
        StructureLimits::empty()
            .with_nodes_limit(ResourceLimit::new(JsonResource::Nodes, 2)),
    );
    let mut budget = JsonValueBudget::new(limits);
    let mut deserializer = Deserializer::from_slice(br#"[1,2]"#);

    let error = BudgetedJsonValueSeed::new(&mut budget)
        .deserialize(&mut deserializer)
        .expect_err("the third decoded node should exceed the budget");

    assert!(error.to_string().contains("Nodes"));
    assert_eq!(budget.structure_budget().used_nodes(), 2);
}

#[test]
fn budgeted_value_seed_returns_the_admitted_value() {
    let mut budget = JsonValueBudget::new(JsonValueLimits::empty());
    let mut deserializer = Deserializer::from_slice(br#"{"key":[true]}"#);

    let value = BudgetedJsonValueSeed::new(&mut budget)
        .deserialize(&mut deserializer)
        .expect("the unconfigured budget should admit the value");

    assert_eq!(value, json!({"key": [true]}));
}
