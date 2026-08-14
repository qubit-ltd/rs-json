// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use std::error::Error;

use qubit_budget::json::JsonDecodeLimits;
use qubit_budget::json::JsonDecodeSession;
use qubit_json::text::JsonDecodeError;
use qubit_json::text::decode_slice;
use qubit_json::text::inspect;

/// Verifies that strict text decoding uses the operation-specific error API.
#[test]
fn test_decode_slice_returns_a_typed_value() {
    let mut session = JsonDecodeSession::owned(JsonDecodeLimits::empty());
    let value: bool =
        decode_slice(b"true", &mut session).expect("valid JSON decodes");
    assert!(value);
}

/// Verifies that strict typed failures do not expose serde input fragments.
#[test]
fn test_decode_slice_redacts_serde_input_details() {
    let mut session = JsonDecodeSession::owned(JsonDecodeLimits::empty());
    let error = decode_slice::<u64, _, _>(br#""TOP_SECRET""#, &mut session)
        .expect_err("a JSON string cannot deserialize into u64");

    assert!(matches!(error, JsonDecodeError::Deserialize(_)));
    assert!(!error.to_string().contains("TOP_SECRET"));
    assert!(error.source().is_none());
    assert!(!format!("{error:?}").contains("TOP_SECRET"));
}

/// Verifies strict decoding exercises valid and invalid lexical boundaries.
#[test]
fn test_decode_slice_checks_lexical_container_and_token_edges() {
    let valid_inputs: &[&[u8]] = &[
        br#" {"items":[true,null,-12.30e+2,"a\n","a\u0041\uD834\uDD1E","\u00e9"]} "#,
        br#"[]"#,
        br#"{}"#,
    ];
    for input in valid_inputs {
        let mut session = JsonDecodeSession::owned(JsonDecodeLimits::empty());
        decode_slice::<serde_json::Value, _, _>(input, &mut session)
            .expect("valid lexical input should decode");
    }

    let invalid_inputs: &[&[u8]] = &[
        b"",
        b"{",
        b"[1 2]",
        b"[1,]",
        br#"{"a" 1}"#,
        br#"{"a":1 "b":2}"#,
        b"{1:2}",
        br#""\q""#,
        br#""\u12x4""#,
        br#""\uD800""#,
        br#""\uDC00""#,
        b"1x",
        b"1e",
        b"1.",
        b"truex",
        b"true false",
        b"\"\xff\"",
        b"\"\xe0\x80\x80\"",
        b"\"\xe2\x82\"",
    ];
    for input in invalid_inputs {
        let mut session = JsonDecodeSession::owned(JsonDecodeLimits::empty());
        let _ = decode_slice::<serde_json::Value, _, _>(input, &mut session)
            .expect_err("invalid lexical input should be rejected");
    }
}

/// Verifies strict inspection charges and validates input without decoding it.
#[test]
fn test_inspect_validates_input_with_operation_specific_errors() {
    let mut session = JsonDecodeSession::owned(JsonDecodeLimits::empty());
    inspect(br#"{"key":[true]}"#, &mut session)
        .expect("valid JSON should pass lexical inspection");

    let mut session = JsonDecodeSession::owned(JsonDecodeLimits::empty());
    let error = inspect(b"[1,]", &mut session)
        .expect_err("invalid JSON should fail lexical inspection");
    assert!(matches!(error, JsonDecodeError::Syntax(_)));
}
