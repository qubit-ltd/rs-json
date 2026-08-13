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
use qubit_json::text::JsonDeserializeError;
use qubit_json::text::JsonDeserializeErrorCategory;
use qubit_json::text::decode_slice;

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

/// Verifies that serde errors convert to safe strict-decoder metadata.
#[test]
fn test_deserialize_error_conversion_redacts_input_details() {
    let source = serde_json::from_slice::<u64>(br#""TOP_SECRET""#)
        .expect_err("a JSON string cannot deserialize into u64");
    let error = JsonDeserializeError::from(source);

    assert_eq!(error.category(), JsonDeserializeErrorCategory::Data);
    assert_eq!(error.line(), 1);
    assert!(!error.to_string().contains("TOP_SECRET"));
}
