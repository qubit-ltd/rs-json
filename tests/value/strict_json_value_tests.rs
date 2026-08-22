// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
// =============================================================================
//! Verifies duplicate-key rejection through `StrictJsonValue`.

use qubit_json::value::StrictJsonValue;
use qubit_json::value::StrictJsonValueSeed;
use serde::de::DeserializeSeed;
use serde_json::Deserializer;
use serde_json::from_str;
use serde_json::json;

/// Verifies strict values retain valid nested JSON documents.
#[test]
fn test_strict_json_value_decodes_unique_nested_keys() {
    let value: StrictJsonValue =
        from_str(r#"{"outer":{"inner":1},"items":[true,null]}"#).expect("unique object keys must decode");

    assert_eq!(
        value.into_inner(),
        json!({"outer": {"inner": 1}, "items": [true, null]})
    );
}

/// Verifies strict values reject repeated keys at every object depth.
#[test]
fn test_strict_json_value_rejects_nested_duplicate_keys() {
    let error = from_str::<StrictJsonValue>(r#"{"outer":{"key":1,"key":2}}"#)
        .expect_err("a repeated nested key must be rejected");

    assert!(error.to_string().contains("duplicate JSON object key 'key'"));
}

/// Verifies the strict seed is reusable with a caller-owned deserializer.
#[test]
fn test_strict_json_value_seed_rejects_duplicate_keys() {
    let mut deserializer = Deserializer::from_str(r#"{"key":1,"key":2}"#);
    let error = StrictJsonValueSeed::new()
        .deserialize(&mut deserializer)
        .expect_err("the strict seed must reject duplicate keys");

    assert!(error.to_string().contains("duplicate JSON object key 'key'"));
}
