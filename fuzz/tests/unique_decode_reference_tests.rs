// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Deterministic regressions for the independent unique-key reference.

use qubit_json_fuzz::unique_decode_reference::parse_unique_literal_value;
use serde_json::Number;
use serde_json::Value;

#[test]
fn test_unique_reference_preserves_number_semantics() {
    for (input, expected) in [
        ("1.5", Number::from_f64(1.5).unwrap()),
        ("-0", Number::from_f64(-0.0).unwrap()),
        ("1e2", Number::from_f64(100.0).unwrap()),
        ("-9223372036854775808", Number::from(i64::MIN)),
        ("18446744073709551615", Number::from(u64::MAX)),
    ] {
        let actual = parse_unique_literal_value(input.as_bytes()).unwrap();
        assert_eq!(actual, Value::Number(expected.clone()), "{input}");
        let actual = actual.as_number().unwrap();
        assert_eq!(actual.is_f64(), expected.is_f64(), "{input}");
        if input == "-0" {
            assert!(actual.as_f64().unwrap().is_sign_negative());
        }
    }
}

#[test]
fn test_unique_reference_preserves_private_key_objects_and_rejects_out_of_contract_inputs() {
    let private_number = parse_unique_literal_value(br#"{"$serde_json::private::Number":"1.5"}"#).unwrap();
    assert_eq!(
        private_number["$serde_json::private::Number"],
        Value::String("1.5".to_owned()),
    );
    let nested = parse_unique_literal_value(br#"{"nested":{"$serde_json::private::Number":"1.5"}}"#).unwrap();
    assert_eq!(
        nested["nested"]["$serde_json::private::Number"],
        Value::String("1.5".to_owned())
    );
    let with_other_field = parse_unique_literal_value(br#"{"other":1,"$serde_json::private::Number":"1.5"}"#).unwrap();
    assert_eq!(with_other_field["other"], Value::Number(Number::from(1)));
    assert_eq!(
        with_other_field["$serde_json::private::Number"],
        Value::String("1.5".to_owned()),
    );
    let raw_value = parse_unique_literal_value(br#"{"$serde_json::private::RawValue":"1.5"}"#).unwrap();
    assert_eq!(
        raw_value["$serde_json::private::RawValue"],
        Value::String("1.5".to_owned())
    );
    for input in [
        br#"{"x":1,"\u0078":2}"#.as_slice(),
        b"18446744073709551616",
        b"-9223372036854775809",
        b"1e400",
        b"1e9999",
        b"[1,]",
        b"null true",
        br#""\uD800""#,
    ] {
        assert!(
            parse_unique_literal_value(input).is_err(),
            "{}",
            String::from_utf8_lossy(input)
        );
    }
}
