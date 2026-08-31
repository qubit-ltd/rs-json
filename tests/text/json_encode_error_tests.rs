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
use qubit_json::encode::JsonSerializationError;
use qubit_json::encode::JsonSerializationErrorCategory;
use qubit_json::encode::JsonSerializationErrorKind;

/// Verifies that Serde failures retain the encoding error category.
#[test]
fn test_serialize_error_variant_is_distinct() {
    let source = JsonSerializationError::new(JsonSerializationErrorKind::CustomSerialization);
    let error = JsonEncodeError::<(), usize>::Serialize(source);

    assert!(matches!(error, JsonEncodeError::Serialize(_)));
}

/// Verifies that invalid raw JSON retains stable lexical diagnostics.
#[test]
fn test_invalid_raw_json_preserves_syntax_error_details() {
    let syntax_error = JsonSyntaxError::new(19, 3, 7, JsonSyntaxErrorReason::InvalidEscape);
    let error = JsonEncodeError::<(), usize>::InvalidRawJson(syntax_error);
    let JsonEncodeError::InvalidRawJson(syntax_error) = error else {
        panic!("expected an invalid raw JSON error");
    };

    assert_eq!(syntax_error.reason(), JsonSyntaxErrorReason::InvalidEscape);
    assert_eq!(syntax_error.offset(), 19);
    assert_eq!(syntax_error.line(), 3);
    assert_eq!(syntax_error.column(), 7);
}

/// Verifies Serde custom failures discard arbitrary diagnostic text.
#[test]
fn test_serde_custom_error_redacts_message() {
    const SECRET: &str = "SERIALIZATION_SECRET";
    let error = <JsonSerializationError as serde::ser::Error>::custom(SECRET);

    assert_eq!(error.kind(), JsonSerializationErrorKind::CustomSerialization);
    assert_eq!(error.category(), JsonSerializationErrorCategory::Custom);
    assert!(!error.to_string().contains(SECRET));
    assert!(!format!("{error:?}").contains(SECRET));
    assert!(error.source().is_none());
}
