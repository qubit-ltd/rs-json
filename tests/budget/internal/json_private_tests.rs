// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Regression tests for serde_json private serializer shapes.

use qubit_budget::json::JsonEncodeLimits;
use qubit_budget::json::JsonEncodeSession;
use qubit_json::text::encode_to_vec;
use serde_json::Number;
use serde_json::from_str;
use serde_json::value::RawValue;

/// Ensures arbitrary-precision numbers and raw fragments stay budget-aware.
#[test]
fn test_private_serde_json_shapes_encode_through_budget() {
    let number: Number = from_str("12345678901234567890")
        .expect("arbitrary-precision number should parse");
    let raw = RawValue::from_string(String::from("{\"ok\":true}"))
        .expect("raw JSON should parse");
    let mut session = JsonEncodeSession::owned(JsonEncodeLimits::empty());

    let output = encode_to_vec(&(&number, &raw), &mut session)
        .expect("private serde_json shapes should encode");

    assert_eq!(output, br#"[12345678901234567890,{"ok":true}]"#);
}
