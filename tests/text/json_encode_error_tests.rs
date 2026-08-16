// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use std::error::Error;

use qubit_json::decode::JsonSyntaxError;
use qubit_json::decode::JsonSyntaxErrorReason;
use qubit_json::encode::JsonEncodeError;
use serde_json::Value;
use serde_json::from_str;

/// Verifies that Serde failures retain the encoding error category.
#[test]
fn test_serialize_error_variant_is_distinct() {
    let error = from_str::<Value>("not-json")
        .expect_err("fixture must be invalid JSON");
    let error = JsonEncodeError::<(), usize>::Serialize(error);

    assert!(matches!(error, JsonEncodeError::Serialize(_)));
}

/// Verifies that invalid raw JSON retains stable lexical diagnostics.
#[test]
fn test_invalid_raw_json_preserves_syntax_error_details() {
    let syntax_error =
        JsonSyntaxError::new(19, 3, 7, JsonSyntaxErrorReason::InvalidEscape);
    let error = JsonEncodeError::<(), usize>::InvalidRawJson(syntax_error);
    let JsonEncodeError::InvalidRawJson(syntax_error) = error else {
        panic!("expected an invalid raw JSON error");
    };

    assert_eq!(syntax_error.reason(), JsonSyntaxErrorReason::InvalidEscape);
    assert_eq!(syntax_error.offset(), 19);
    assert_eq!(syntax_error.line(), 3);
    assert_eq!(syntax_error.column(), 7);
}

/// Verifies Serde custom failures are preserved as serialization errors.
#[test]
fn test_serde_custom_error_preserves_message() {
    let error = <JsonEncodeError<(), usize> as serde::ser::Error>::custom(
        "fixture failure",
    );

    assert!(matches!(error, JsonEncodeError::Serialize(_)));
    assert_eq!(
        error.to_string(),
        "JSON serialization failed: fixture failure"
    );
    assert!(error.source().is_some());
}
