// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Verifies duplicate-key rejection through `DuplicateKeyRejectingJsonValue`.

use qubit_json::value::DuplicateKeyRejectingJsonValue;
use qubit_json::value::DuplicateKeyRejectingJsonValueSeed;
use serde::de::DeserializeSeed;
use serde_json::Deserializer;
use serde_json::from_str;
use serde_json::json;

/// Verifies strict values retain valid nested JSON documents.
#[test]
fn test_duplicate_key_rejecting_json_value_decodes_unique_nested_keys() {
    let value: DuplicateKeyRejectingJsonValue =
        from_str(r#"{"outer":{"inner":1},"items":[true,null]}"#)
            .expect("unique object keys must decode");

    assert_eq!(
        value.into_inner(),
        json!({"outer": {"inner": 1}, "items": [true, null]})
    );
}

/// Verifies strict values reject repeated keys at every object depth.
#[test]
fn test_duplicate_key_rejecting_json_value_rejects_nested_duplicate_keys() {
    let error = from_str::<DuplicateKeyRejectingJsonValue>(r#"{"outer":{"key":1,"key":2}}"#)
        .expect_err("a repeated nested key must be rejected");

    assert!(error.to_string().contains("duplicate JSON object key"));
    assert!(!error.to_string().contains("'key'"));
}

/// Verifies the strict seed is reusable with a caller-owned deserializer.
#[test]
fn test_duplicate_key_rejecting_json_value_seed_rejects_duplicate_keys() {
    let mut deserializer = Deserializer::from_str(r#"{"key":1,"key":2}"#);
    let error = DuplicateKeyRejectingJsonValueSeed::new()
        .deserialize(&mut deserializer)
        .expect_err("the strict seed must reject duplicate keys");

    assert!(error.to_string().contains("duplicate JSON object key"));
    assert!(!error.to_string().contains("'key'"));
}

/// Verifies a duplicate key is rejected before its malformed value is read.
#[test]
fn test_duplicate_key_rejecting_json_value_stops_before_duplicate_value() {
    const SECRET_KEY: &str = "sensitive-key";
    let input = format!(r#"{{"{SECRET_KEY}":1,"{SECRET_KEY}":invalid}}"#);

    let error = from_str::<DuplicateKeyRejectingJsonValue>(&input)
        .expect_err("the repeated key must fail before its value is decoded");

    assert!(error.to_string().contains("duplicate JSON object key"));
    assert!(!error.to_string().contains(SECRET_KEY));
    assert!(!error.to_string().contains("expected value"));
}

/// Verifies serde_json's former private number marker has no special meaning.
#[test]
fn test_duplicate_key_rejecting_json_value_preserves_private_number_marker_object() {
    const MARKER: &str = concat!("$", "serde_json", "::private::Number");
    let input = format!(r#"{{"{MARKER}":"123"}}"#);
    let value: DuplicateKeyRejectingJsonValue =
        from_str(&input).expect("the marker-shaped object must remain valid JSON");

    assert_eq!(value.into_inner(), json!({MARKER: "123"}),);
}
