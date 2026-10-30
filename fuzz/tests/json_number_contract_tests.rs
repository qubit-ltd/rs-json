// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests the independent JSON number-contract classifier.

use qubit_json::encode::JsonIntegerSignedness;
use qubit_json::encode::JsonSerializationErrorKind;
use qubit_json_fuzz::json_number_contract::normalize_reference_value;
use qubit_json_fuzz::json_number_contract::numbers_fit_contract;
use qubit_json_fuzz::json_number_contract::parse_strict_reference_number;
use serde_json::Value;
use serde_json::from_slice;

/// Verifies every inclusive numeric boundary is admitted.
#[test]
fn test_numbers_fit_contract_accepts_supported_boundaries() {
    let input = br#"{
        "signed": -9223372036854775808,
        "unsigned": 18446744073709551615,
        "fraction": 1.7976931348623157e308,
        "numeric text": "18446744073709551616"
    }"#;

    from_slice::<Value>(input).expect("the reference input must be valid JSON");
    assert!(numbers_fit_contract(input));
}

/// Verifies integers and floats outside the supported ranges are rejected.
#[test]
fn test_numbers_fit_contract_rejects_unsupported_ranges() {
    for input in [&b"-9223372036854775809"[..], &b"18446744073709551616"[..]] {
        from_slice::<Value>(input).expect("serde_json must accept the reference input");
        assert!(
            !numbers_fit_contract(input),
            "number must be outside the contract: {input:?}"
        );
    }
    assert!(!numbers_fit_contract(b"1e400"));
}

#[test]
fn test_strict_reference_number_keeps_the_product_range_in_all_features() {
    assert_eq!(parse_strict_reference_number("1.25").unwrap().as_f64(), Some(1.25));
    assert_eq!(parse_strict_reference_number("1e2").unwrap().as_f64(), Some(100.0));
    assert!(
        parse_strict_reference_number("-0")
            .unwrap()
            .as_f64()
            .unwrap()
            .is_sign_negative()
    );
    assert_eq!(
        parse_strict_reference_number("1e400"),
        Err(JsonSerializationErrorKind::NonFiniteFloat)
    );
    assert_eq!(
        parse_strict_reference_number("01"),
        Err(JsonSerializationErrorKind::InvalidNumberRepresentation)
    );
    assert_eq!(
        parse_strict_reference_number(" 1"),
        Err(JsonSerializationErrorKind::InvalidNumberRepresentation)
    );
    assert_eq!(
        parse_strict_reference_number("1 "),
        Err(JsonSerializationErrorKind::InvalidNumberRepresentation)
    );
    assert_eq!(
        parse_strict_reference_number(i64::MIN.to_string().as_str())
            .unwrap()
            .as_i64(),
        Some(i64::MIN)
    );
    assert_eq!(
        parse_strict_reference_number(u64::MAX.to_string().as_str())
            .unwrap()
            .as_u64(),
        Some(u64::MAX)
    );
    assert_eq!(
        parse_strict_reference_number("-9223372036854775809"),
        Err(JsonSerializationErrorKind::IntegerOutOfRange {
            signedness: JsonIntegerSignedness::Signed,
        })
    );
    assert_eq!(
        parse_strict_reference_number("18446744073709551616"),
        Err(JsonSerializationErrorKind::IntegerOutOfRange {
            signedness: JsonIntegerSignedness::Unsigned,
        })
    );
}

#[test]
fn test_reference_normalization_uses_fixed_numeric_projections() {
    let value: Value = serde_json::from_str(r#"{"n":1e2}"#).unwrap();
    assert_eq!(
        normalize_reference_value(&value).unwrap(),
        serde_json::json!({"n": 100.0})
    );
    let value: Value = serde_json::from_str("[-0,1.000]").unwrap();
    let normalized = normalize_reference_value(&value).unwrap();
    assert_eq!(normalized[0].as_f64(), Some(-0.0));
    assert!(normalized[0].as_f64().unwrap().is_sign_negative());
    assert_eq!(normalized[1].as_f64(), Some(1.0));
}
