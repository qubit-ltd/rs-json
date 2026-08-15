// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests JSON value visitor construction through the public seed.

use qubit_budget::json::JsonValueLimits;
use qubit_json::value::JsonValueSeed;
use serde::de::DeserializeSeed;
use serde_json::Deserializer;
use serde_json::json;

/// Verifies the value visitor materializes nested scalar JSON values.
#[test]
fn test_json_value_visitor_builds_nested_value() {
    let mut budget = JsonValueLimits::empty().budget();
    let mut transaction = budget.transaction();
    let mut deserializer = Deserializer::from_slice(br#"{"key":[true,3]}"#);
    let value = JsonValueSeed::new(&mut transaction)
        .deserialize(&mut deserializer)
        .expect("JSON value seed should build the nested value");

    assert_eq!(value, json!({"key": [true, 3]}));
}
