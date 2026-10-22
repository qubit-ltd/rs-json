// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use qubit_budget::json::JsonResource;
use qubit_budget::json::JsonValueBudget;
use qubit_budget::json::JsonValueLimits;
use qubit_json::value::AccountingJsonValueSeed;
use qubit_json::value::DuplicateKeyRejectingJsonValueSeed;
use qubit_json::value::JsonValueEncoder;
use qubit_json::value::traverse::JsonTreeBudgetTracker;
use serde::de::DeserializeSeed;
use serde::de::value::I128Deserializer;
use serde::de::value::U128Deserializer;
use serde_json::Value;

#[test]
fn test_tree_number_measurement_uses_the_stored_representation() {
    for text in ["-0", "1.5", "18446744073709551615"] {
        let value: Value = serde_json::from_str(text).unwrap();
        let expected = serde_json::to_vec(value.as_number().unwrap()).unwrap().len();
        let limits = JsonValueLimits::<JsonResource, usize>::builder()
            .max_number_bytes(expected)
            .build();
        JsonTreeBudgetTracker::new(limits)
            .account(&value)
            .expect("the exact stored number representation should fit");
    }
}

#[test]
fn test_value_encoder_keeps_marker_named_objects_as_objects() {
    let value = JsonValueEncoder::new()
        .encode(&serde_json::json!({"$serde_json::private::Number": "1.5"}))
        .unwrap();
    assert!(value.is_object());
}

#[test]
fn test_seed_entry_points_enforce_the_64_bit_integer_union() {
    let mut budget = JsonValueBudget::new(JsonValueLimits::<JsonResource, usize>::default());
    let mut transaction = budget.transaction();
    let accepted = AccountingJsonValueSeed::new(&mut transaction)
        .deserialize(U128Deserializer::<serde_json::Error>::new(u64::MAX as u128))
        .unwrap();
    assert_eq!(accepted.as_u64(), Some(u64::MAX));
    assert!(AccountingJsonValueSeed::new(&mut transaction)
        .deserialize(U128Deserializer::<serde_json::Error>::new(u128::MAX))
        .is_err());

    assert!(DuplicateKeyRejectingJsonValueSeed::new()
        .deserialize(I128Deserializer::<serde_json::Error>::new(i64::MIN as i128 - 1))
        .is_err());
}

#[cfg(feature = "arbitrary_precision")]
#[test]
fn test_arbitrary_precision_tree_numbers_are_counted_without_float_conversion() {
    for text in ["340282366920938463463374607431768211455", "1e400"] {
        let value: Value = serde_json::from_str(text).unwrap();
        let expected = serde_json::to_vec(value.as_number().unwrap()).unwrap().len();
        let exact_limits = JsonValueLimits::<JsonResource, usize>::builder()
            .max_number_bytes(expected)
            .max_nodes(4)
            .build();
        JsonTreeBudgetTracker::new(exact_limits)
            .account(&value)
            .expect("the stored number representation should fit its exact byte limit");

        let limited = expected - 1;
        let limits = JsonValueLimits::<JsonResource, usize>::builder()
            .max_number_bytes(limited)
            .max_nodes(4)
            .build();
        let mut tracker = JsonTreeBudgetTracker::new(limits);
        let error = tracker.account(&value).unwrap_err();
        assert_eq!(error.resource(), &JsonResource::NumberBytes);
        assert_eq!(
            error
                .budget_error()
                .expect("number-byte rejection is a budget limit error")
                .exact_observed(),
            Some(expected)
        );
        assert_eq!(tracker.budget().used_nodes(), Some(0));
    }
}
