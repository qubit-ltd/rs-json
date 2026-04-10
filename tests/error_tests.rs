/*******************************************************************************
 *
 *    Copyright (c) 2026.
 *    Haixing Hu, Qubit Co. Ltd.
 *
 *    All rights reserved.
 *
 ******************************************************************************/
//! Tests for the public `JsonDecodeError` type in `error.rs`.
//!
//! Author: Haixing Hu

use qubit_json::{JsonDecodeError, JsonDecodeErrorKind, JsonTopLevelKind, LenientJsonDecoder};

#[test]
fn test_error_display_for_empty_input_uses_message() {
    let error = JsonDecodeError {
        kind: JsonDecodeErrorKind::EmptyInput,
        message: "JSON input is empty after normalization".to_string(),
        expected_top_level: None,
        actual_top_level: None,
        line: None,
        column: None,
    };
    assert_eq!(error.to_string(), "JSON input is empty after normalization");
    assert!(std::error::Error::source(&error).is_none());
}

#[test]
fn test_error_display_for_top_level_mismatch_uses_expected_and_actual() {
    let error = JsonDecodeError {
        kind: JsonDecodeErrorKind::UnexpectedTopLevel,
        message: "Unexpected JSON top-level type: expected object, got array".to_string(),
        expected_top_level: Some(JsonTopLevelKind::Object),
        actual_top_level: Some(JsonTopLevelKind::Array),
        line: None,
        column: None,
    };
    assert_eq!(
        error.to_string(),
        "Unexpected JSON top-level type: expected object, got array"
    );
}

#[test]
fn test_error_display_for_invalid_json_includes_location() {
    let decoder = LenientJsonDecoder::default();
    let error = decoder
        .decode_value("{")
        .expect_err("invalid JSON should return a parse error");
    assert_eq!(error.kind, JsonDecodeErrorKind::InvalidJson);
    assert!(error.line.is_some());
    assert!(error.column.is_some());
    assert!(error.to_string().contains("Failed to parse JSON:"));
    assert!(error.to_string().contains("line"));
}

#[test]
fn test_error_display_for_deserialize_error_uses_context_message() {
    let decoder = LenientJsonDecoder::default();
    let error = decoder
        .decode::<u64>("\"text\"")
        .expect_err("string JSON should not deserialize into u64");
    assert_eq!(error.kind, JsonDecodeErrorKind::Deserialize);
    assert!(error
        .to_string()
        .contains("Failed to deserialize JSON value:"));
}
