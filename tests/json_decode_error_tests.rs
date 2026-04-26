/*******************************************************************************
 *
 *    Copyright (c) 2026.
 *    Haixing Hu, Qubit Co. Ltd.
 *
 *    All rights reserved.
 *
 ******************************************************************************/
//! Tests for the public [`qubit_json::JsonDecodeError`] type in
//! `json_decode_error.rs`.
//!
//! Author: Haixing Hu

use qubit_json::{
    JsonDecodeError, JsonDecodeErrorKind, JsonDecodeStage, JsonTopLevelKind, LenientJsonDecoder,
};

#[test]
fn test_error_display_for_empty_input_uses_message() {
    let error = JsonDecodeError {
        kind: JsonDecodeErrorKind::EmptyInput,
        stage: JsonDecodeStage::Normalize,
        message: "JSON input is empty after normalization".to_string(),
        expected_top_level: None,
        actual_top_level: None,
        line: None,
        column: None,
        input_bytes: None,
        max_input_bytes: None,
    };
    assert_eq!(error.to_string(), "JSON input is empty after normalization");
    assert!(std::error::Error::source(&error).is_none());
}

#[test]
fn test_error_display_for_input_too_large_uses_message() {
    let error = JsonDecodeError {
        kind: JsonDecodeErrorKind::InputTooLarge,
        stage: JsonDecodeStage::Normalize,
        message: "JSON input is too large: 8 bytes exceed configured limit 7 bytes".to_string(),
        expected_top_level: None,
        actual_top_level: None,
        line: None,
        column: None,
        input_bytes: Some(8),
        max_input_bytes: Some(7),
    };
    assert_eq!(
        error.to_string(),
        "JSON input is too large: 8 bytes exceed configured limit 7 bytes"
    );
}

#[test]
fn test_error_display_for_top_level_mismatch_uses_expected_and_actual() {
    let error = JsonDecodeError {
        kind: JsonDecodeErrorKind::UnexpectedTopLevel,
        stage: JsonDecodeStage::TopLevelCheck,
        message: "Unexpected JSON top-level type: expected object, got array".to_string(),
        expected_top_level: Some(JsonTopLevelKind::Object),
        actual_top_level: Some(JsonTopLevelKind::Array),
        line: None,
        column: None,
        input_bytes: None,
        max_input_bytes: None,
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
    assert_eq!(error.stage, JsonDecodeStage::Parse);
    assert!(error.line.is_some());
    assert!(error.column.is_some());
    assert!(error.input_bytes.is_some());
    assert!(error.to_string().contains("Failed to parse JSON:"));
    assert!(error.to_string().contains("line"));
}

#[test]
fn test_error_display_for_parse_or_deserialize_without_location_uses_message() {
    let error = JsonDecodeError {
        kind: JsonDecodeErrorKind::Deserialize,
        stage: JsonDecodeStage::Deserialize,
        message: "Failed to deserialize JSON value: invalid type".to_string(),
        expected_top_level: None,
        actual_top_level: None,
        line: None,
        column: None,
        input_bytes: Some(12),
        max_input_bytes: None,
    };
    assert_eq!(
        error.to_string(),
        "Failed to deserialize JSON value: invalid type"
    );
}

#[test]
fn test_error_display_for_deserialize_error_uses_context_message() {
    let decoder = LenientJsonDecoder::default();
    let error = decoder
        .decode::<u64>("\"text\"")
        .expect_err("string JSON should not deserialize into u64");
    assert_eq!(error.kind, JsonDecodeErrorKind::Deserialize);
    assert_eq!(error.stage, JsonDecodeStage::Deserialize);
    assert!(error.input_bytes.is_some());
    assert!(
        error
            .to_string()
            .contains("Failed to deserialize JSON value:")
    );
}
