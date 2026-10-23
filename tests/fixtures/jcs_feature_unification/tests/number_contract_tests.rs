// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

#[cfg(feature = "arbitrary_precision")]
use qubit_budget::ResourceLimit;
#[cfg(feature = "arbitrary_precision")]
use qubit_budget::json::JsonEncodeLimits;
#[cfg(feature = "arbitrary_precision")]
use qubit_budget::json::JsonEncodeSession;
use qubit_budget::json::JsonResource;
use qubit_budget::json::JsonValueBudget;
use qubit_budget::json::JsonValueLimits;
use qubit_json::decode::JsonDecoder;
#[cfg(feature = "arbitrary_precision")]
use qubit_json::encode::JsonEncodeErrorKind;
use qubit_json::encode::JsonEncoder;
use qubit_json::value::AccountingJsonValueSeed;
use qubit_json::value::DuplicateKeyRejectingJsonValueSeed;
use qubit_json::value::JsonValueEncoder;
use qubit_json::value::traverse::JsonTreeBudgetTracker;
use serde::de::DeserializeSeed;
use serde::de::value::I128Deserializer;
use serde::de::value::U128Deserializer;
use serde_json::Number;
use serde_json::Value;

#[test]
fn test_real_number_zero_exponents_and_underflow() {
    for (text, expected) in [
        ("0e1", 0.0_f64),
        ("0e+10", 0.0),
        ("0.0e-10", 0.0),
        ("-0e1", -0.0),
        ("-0.0e+10", -0.0),
        ("1e-400", 0.0),
        ("-1e-400", -0.0),
        ("5e-324", f64::from_bits(1)),
        ("-5e-324", -f64::from_bits(1)),
    ] {
        let number: Number = serde_json::from_str(text).expect(text);
        let value = JsonValueEncoder::new().encode(&number).expect(text);
        assert_eq!(
            value.as_f64().expect(text).to_bits(),
            expected.to_bits(),
            "{text}"
        );

        let bytes = JsonEncoder::unlimited().to_vec(&number).expect(text);
        let decoded: Value = JsonDecoder::unlimited().decode_utf8(&bytes).expect(text);
        assert_eq!(
            decoded.as_f64().expect(text).to_bits(),
            expected.to_bits(),
            "{text}"
        );
    }
}

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

#[cfg(feature = "arbitrary_precision")]
#[test]
fn test_arbitrary_precision_real_numbers_still_enforce_strict_ranges() {
    for text in ["1e400", "18446744073709551616"] {
        let number: Number =
            serde_json::from_str(text).expect("arbitrary-precision Number must parse");
        let error = JsonEncoder::unlimited()
            .to_vec(&number)
            .expect_err("ordinary encoding must reject values outside the strict range");
        assert_eq!(error.kind(), JsonEncodeErrorKind::Serialize, "{text}");
    }
}

#[cfg(feature = "arbitrary_precision")]
#[test]
fn test_arbitrary_precision_real_number_bytes_are_budgeted() {
    let number: Number = serde_json::from_str("0e10").expect("the number must parse");
    let limits = JsonEncodeLimits::builder()
        .output_bytes_limit(ResourceLimit::new(JsonResource::OutputBytes, 256_usize))
        .value_limits(
            JsonValueLimits::builder()
                .max_number_bytes(3_usize)
                .max_nodes(64_usize)
                .build(),
        )
        .build();
    let mut encoder = JsonEncoder::new(JsonEncodeSession::from_limits(limits));
    let error = encoder
        .to_vec(&number)
        .expect_err("four-byte number lexeme must exceed the three-byte budget");
    assert_eq!(error.kind(), JsonEncodeErrorKind::Budget);
    assert_eq!(encoder.session().value_budget().used_nodes(), Some(0));
}
