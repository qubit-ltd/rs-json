// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
// =============================================================================
//! Regressions for literal JSON reference parsing used by fuzz oracles.

use qubit_json_fuzz::json_reference::parse_literal_value;
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

/// The literal reference must still reject invalid syntax and Unicode.
#[test]
fn test_reference_rejects_invalid_json() {
    for input in [br#""\uD800""#.as_slice(), b"[1,]", b"null true", b"1e9999"] {
        assert!(parse_literal_value(input).is_err(), "input: {input:?}");
    }
}
