// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Regressions for literal JSON reference parsing used by fuzz oracles.

use qubit_json::encode::JcsEncoder;
use qubit_json_fuzz::json_reference::parse_literal_value;
use qubit_json_fuzz::json_reference::roundtrip_literal_value;
use serde_json::Number;
use serde_json::Value;

/// Ordinary JSON values retain the reference parser's scalar and container
/// data.
#[test]
fn test_reference_preserves_ordinary_values() {
    let input = br#"[null,true,false,-1,18446744073709551615,0.25,"\u0061",{"a":[1]}]"#;
    let expected: Value = serde_json::from_slice(input).expect("ordinary JSON parses");
    assert_eq!(parse_literal_value(input).expect("literal reference parses"), expected);
}

/// Private protocol-looking keys remain ordinary JSON object members.
#[test]
fn test_reference_preserves_private_raw_value_keys() {
    for input in [
        br#"{"$serde_json::private::RawValue":""}"#.as_slice(),
        br#"{"$serde_json::private::RawValue":"null"}"#,
        br#"{"$serde_json::private::RawValue":8}"#,
    ] {
        let value = parse_literal_value(input).expect("ordinary object keys must parse");
        assert!(value.is_object());
        assert!(value.get("$serde_json::private::RawValue").is_some());
    }
}

/// Encoding sorts keys; moving a private-looking key first cannot change data.
#[test]
fn test_reference_roundtrip_preserves_nested_private_key_after_sorting() {
    let input = br#"{"":{"z":4,"$serde_json::private::RawValue":8}}"#;
    let original: Value = serde_json::from_slice(input).expect("original key order parses");
    let encoded = serde_json::to_vec(&original).expect("value encodes");
    let decoded = parse_literal_value(&encoded).expect("sorted object remains JSON");
    assert_eq!(decoded, original);
}

/// Ordinary Value deserialization can reinterpret a private-looking key.
#[test]
fn test_reference_roundtrip_preserves_raw_value_key_in_object_and_array() {
    let mut object = serde_json::Map::new();
    object.insert(
        "$serde_json::private::RawValue".to_owned(),
        Value::String("null".to_owned()),
    );
    let value = Value::Object(object);
    assert_eq!(roundtrip_literal_value(&value).expect("object roundtrip"), value);

    let wrapped = Value::Array(vec![value.clone()]);
    assert_eq!(roundtrip_literal_value(&wrapped).expect("array roundtrip"), wrapped);
}

/// JCS encoding retains private-looking keys as ordinary object members.
#[test]
fn test_jcs_encoder_preserves_private_raw_value_key_objects() {
    for (input, expected) in [
        (
            br#"{"$serde_json::private::RawValue":"null"}"#.as_slice(),
            br#"{"$serde_json::private::RawValue":"null"}"#.as_slice(),
        ),
        (
            br#"{"$serde_json::private::RawValue":8}"#.as_slice(),
            br#"{"$serde_json::private::RawValue":8}"#.as_slice(),
        ),
    ] {
        let value = parse_literal_value(input).expect("literal object parses");
        let encoded = JcsEncoder::unlimited().to_vec(&value).expect("JCS encoding succeeds");
        assert_eq!(encoded, expected);
    }
}

/// The literal reference must still reject invalid syntax and Unicode.
#[test]
fn test_reference_rejects_invalid_json() {
    for input in [br#""\uD800""#.as_slice(), b"[1,]", b"null true"] {
        assert!(parse_literal_value(input).is_err(), "input: {input:?}");
    }
}

/// Numbers retain their token provenance across the feature-specific parser.
#[test]
fn test_literal_reference_preserves_numeric_token_provenance() {
    for token in ["1.5", "1e2", "-0", "-9223372036854775808", "18446744073709551615"] {
        let expected: Number = serde_json::from_str(token).expect("number token parses");
        let actual = parse_literal_value(token.as_bytes()).expect("literal number parses");
        assert_eq!(actual, Value::Number(expected), "{token}");
        assert_eq!(
            roundtrip_literal_value(&actual).expect("number roundtrip"),
            actual,
            "{token}"
        );
        if token == "-0" {
            assert!(actual.as_f64().expect("negative zero is numeric").is_sign_negative());
        }
    }
}

/// Number and RawValue private-looking keys remain ordinary object data.
#[test]
fn test_literal_reference_preserves_private_keys_with_numeric_children() {
    for key in ["$serde_json::private::Number", "$serde_json::private::RawValue"] {
        let input = format!(r#"{{"{key}":"1.5","child":[1.5,{{"{key}":"1e2"}}]}}"#);
        let actual = parse_literal_value(input.as_bytes()).expect("literal object parses");
        assert_eq!(actual[key], Value::String("1.5".to_owned()));
        assert!(actual["child"][0].is_number());
        assert_eq!(actual["child"][1][key], Value::String("1e2".to_owned()));
        assert_eq!(roundtrip_literal_value(&actual).expect("object roundtrip"), actual);
    }
}

/// Recursive reference parsing keeps the established 127-container boundary.
#[test]
fn test_literal_reference_keeps_container_depth_boundary() {
    for (open, close) in [("[", "]"), (r#"{"x":"#, "}")] {
        for depth in [127, 128] {
            let input = format!("{}0{}", open.repeat(depth), close.repeat(depth));
            assert_eq!(parse_literal_value(input.as_bytes()).is_ok(), depth == 127);
        }
    }
}

/// Duplicate decoded keys retain serde_json map overwrite behavior.
#[test]
fn test_literal_reference_retains_decoded_duplicate_key_overwrite() {
    let actual = parse_literal_value(br#"{"x":1,"\u0078":2}"#).expect("duplicate keys parse");
    assert_eq!(actual.as_object().expect("object result").len(), 1);
    assert_eq!(actual["x"], Value::from(2));
}

/// Arbitrary precision preserves numbers outside the default Number domain.
#[cfg(feature = "arbitrary-precision")]
#[test]
fn test_literal_reference_keeps_arbitrary_precision_numeric_storage() {
    for token in ["18446744073709551616", "1e400", "1e9999"] {
        let expected: Number = serde_json::from_str(token).expect("arbitrary number parses");
        let actual = parse_literal_value(token.as_bytes()).expect("literal number parses");
        assert_eq!(actual, Value::Number(expected));
        assert_eq!(roundtrip_literal_value(&actual).expect("number roundtrip"), actual);
    }
}

/// The default Number parser rejects values outside its numeric domain.
#[cfg(not(feature = "arbitrary-precision"))]
#[test]
fn test_literal_reference_rejects_out_of_domain_exponent() {
    assert!(parse_literal_value(b"1e9999").is_err());
}
