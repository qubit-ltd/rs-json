// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests serde_json private serializer shapes through the public encoder.

use qubit_budget::json::JsonEncodeLimits;
use qubit_budget::json::JsonEncodeSession;
use qubit_budget::json::JsonResource;
use serde_json::Number;
use serde_json::from_str;
use serde_json::value::RawValue;

use crate::text::json_encode_test_support::encode;

/// Ensures arbitrary-precision numbers and raw fragments stay budget-aware.
#[test]
fn test_private_serde_json_shapes_encode_through_budget() {
    let number: Number = from_str("12345678901234567890")
        .expect("arbitrary-precision number should parse");
    let raw = RawValue::from_string(String::from("{\"ok\":true}"))
        .expect("raw JSON should parse");
    let mut session = JsonEncodeSession::owned(
        JsonEncodeLimits::<JsonResource, usize>::builder().build(),
    );

    let output = encode(&(&number, &raw), &mut session)
        .expect("private serde_json shapes should encode");

    assert_eq!(output, br#"[12345678901234567890,{"ok":true}]"#);
}

/// Ensures a real arbitrary-precision Number bypasses private map metadata.
#[test]
fn test_real_number_uses_private_number_classification() {
    const NUMBER_TEXT: &str = "12345678901234567890";
    let number: Number =
        from_str(NUMBER_TEXT).expect("arbitrary-precision number should parse");
    let limits = JsonEncodeLimits::<JsonResource, usize>::builder()
        .max_nodes(1)
        .max_map_entries(0)
        .max_key_bytes(0)
        .max_string_bytes(0)
        .max_number_bytes(NUMBER_TEXT.len())
        .build();
    let mut session = JsonEncodeSession::owned(limits);

    let output = encode(&number, &mut session)
        .expect("private Number metadata should not consume object limits");

    assert_eq!(output, NUMBER_TEXT.as_bytes());
}

/// Ensures a real RawValue is budgeted as its represented JSON structure.
#[test]
fn test_real_raw_value_uses_private_raw_value_classification() {
    let raw = RawValue::from_string(String::from("{\"ok\":true}"))
        .expect("raw JSON should parse");
    let limits = JsonEncodeLimits::<JsonResource, usize>::builder()
        .max_nodes(2)
        .max_map_entries(1)
        .max_key_bytes(2)
        .max_string_bytes(0)
        .build();
    let mut session = JsonEncodeSession::owned(limits);

    let output = encode(&raw, &mut session)
        .expect("private RawValue metadata should not consume string limits");

    assert_eq!(output, br#"{"ok":true}"#);
}
