/*******************************************************************************
 *
 *    Copyright (c) 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
//! Tests for the public [`qubit_json::JsonDecodeError`] type in
//! `json_decode_error.rs`.
//!

use qubit_json::{
    JsonDecodeErrorKind, JsonDecodeOptions, JsonDecodeStage, JsonTopLevelKind, LenientJsonDecoder,
};

#[test]
fn test_error_display_for_empty_input_uses_message() {
    let decoder = LenientJsonDecoder::default();
    let error = decoder
        .decode_value("")
        .expect_err("empty input should return a normalization error");
    assert_eq!(error.to_string(), "JSON input is empty after normalization");
    assert!(std::error::Error::source(&error).is_none());
}

#[test]
fn test_error_display_for_input_too_large_uses_message() {
    let decoder = LenientJsonDecoder::new(JsonDecodeOptions {
        max_input_bytes: Some(7),
        ..JsonDecodeOptions::default()
    });
    let error = decoder
        .decode_value("{\"a\": 1}")
        .expect_err("oversized input should return an input-too-large error");
    assert_eq!(
        error.to_string(),
        "JSON input is too large: 8 bytes exceed configured limit 7 bytes"
    );
}

#[test]
fn test_error_display_for_top_level_mismatch_uses_expected_and_actual() {
    let decoder = LenientJsonDecoder::default();
    let error = decoder
        .decode_object::<serde_json::Value>("[]")
        .expect_err("top-level array should fail an object contract");
    assert_eq!(error.expected_top_level, Some(JsonTopLevelKind::Object));
    assert_eq!(error.actual_top_level, Some(JsonTopLevelKind::Array));
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
    assert!(std::error::Error::source(&error).is_some());
}

#[test]
fn test_error_source_for_invalid_json_preserves_serde_error() {
    let decoder = LenientJsonDecoder::default();
    let error = decoder
        .decode_value("{")
        .expect_err("invalid JSON should preserve the parser source error");
    let source = std::error::Error::source(&error)
        .expect("invalid JSON errors should expose the serde_json source");

    assert!(source.to_string().contains("EOF"));
}

#[test]
fn test_error_display_for_parse_or_deserialize_without_location_uses_message() {
    let decoder = LenientJsonDecoder::default();
    let error = decoder
        .decode_object::<u64>("{\"a\":1}")
        .expect_err("deserializing a parsed object into u64 should fail");
    assert_eq!(error.kind, JsonDecodeErrorKind::Deserialize);
    assert_eq!(error.stage, JsonDecodeStage::Deserialize);
    assert_eq!(error.line, None);
    assert_eq!(error.column, None);
    assert!(!error.to_string().contains("line"));
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
    assert!(std::error::Error::source(&error).is_some());
}

#[test]
fn test_error_partial_eq_compares_all_stable_fields() {
    let decoder = LenientJsonDecoder::default();
    let first = decoder
        .decode_value("{\n")
        .expect_err("invalid json should return parse error");
    let second = decoder
        .decode_value("{\n")
        .expect_err("invalid json should return parse error");

    assert_eq!(first, second);

    let third = decoder
        .decode_value("")
        .expect_err("empty input should return normalization error");
    assert_ne!(first, third);
}
