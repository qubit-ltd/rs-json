// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests for the public [`qubit_json::JsonDecodeError`] type.

use serde::Deserialize;

use qubit_json::{
    ErrorPrivacyPolicy,
    JsonDecodeErrorKind,
    JsonDecodeOptions,
    JsonDecodeStage,
    JsonTopLevelKind,
    LenientJsonDecoder,
};

#[derive(Debug, Deserialize)]
enum PublicChoice {
    Allowed,
}

#[test]
fn test_error_display_for_empty_input_uses_message() {
    let error = LenientJsonDecoder::default()
        .decode_value("")
        .expect_err("empty input should return a normalization error");
    assert_eq!(error.to_string(), "JSON input is empty after normalization");
    assert_eq!(error.privacy_policy(), ErrorPrivacyPolicy::Redacted);
    assert_eq!(error.raw_input_bytes(), 0);
    assert_eq!(error.normalized_input_bytes(), None);
    assert!(std::error::Error::source(&error).is_none());
}

#[test]
fn test_error_display_for_input_too_large_uses_message() {
    let decoder = LenientJsonDecoder::new(
        JsonDecodeOptions::default().with_max_input_bytes(Some(7)),
    );
    let error = decoder
        .decode_value("{\"a\": 1}")
        .expect_err("oversized input should return an input-too-large error");
    assert_eq!(
        error.to_string(),
        "JSON input is too large: 8 bytes exceed configured limit 7 bytes"
    );
    assert_eq!(error.raw_input_bytes(), 8);
    assert_eq!(error.max_input_bytes(), Some(7));
    assert_eq!(error.privacy_policy(), ErrorPrivacyPolicy::Redacted);
}

#[test]
fn test_error_exposes_top_level_mismatch_context() {
    let error = LenientJsonDecoder::default()
        .decode_object::<serde_json::Value>("[]")
        .expect_err("top-level array should fail an object contract");
    assert_eq!(error.expected_top_level(), Some(JsonTopLevelKind::Object));
    assert_eq!(error.actual_top_level(), Some(JsonTopLevelKind::Array));
    assert_eq!(error.raw_input_bytes(), 2);
    assert_eq!(error.normalized_input_bytes(), Some(2));
    assert_eq!(error.privacy_policy(), ErrorPrivacyPolicy::Redacted);
    assert_eq!(
        error.to_string(),
        "Unexpected JSON top-level type: expected object, got array"
    );
}

#[test]
fn test_error_exposes_immutable_normalized_diagnostics_without_duplicate_location()
 {
    let error = LenientJsonDecoder::default()
        .decode_value("  {\n")
        .expect_err(
            "incomplete JSON should fail after whitespace normalization",
        );

    assert_eq!(error.kind(), JsonDecodeErrorKind::InvalidJson);
    assert_eq!(error.stage(), JsonDecodeStage::Parse);
    assert_eq!(error.raw_input_bytes(), 4);
    assert_eq!(error.normalized_input_bytes(), Some(1));
    assert_eq!(error.normalized_line(), Some(1));
    assert_eq!(error.normalized_column(), Some(1));
    assert_eq!(error.to_string(), error.message());
    assert!(std::error::Error::source(&error).is_none());
}

#[test]
fn test_error_source_for_invalid_json_preserves_serde_error() {
    let decoder = LenientJsonDecoder::new(
        JsonDecodeOptions::default()
            .with_error_privacy_policy(ErrorPrivacyPolicy::Detailed),
    );
    let error = decoder
        .decode_value("{")
        .expect_err("invalid JSON should preserve the parser source error");
    let source = std::error::Error::source(&error)
        .expect("invalid JSON errors should expose the serde_json source");
    assert!(source.to_string().contains("EOF"));
}

#[test]
fn test_default_error_privacy_redacts_input_derived_serde_details() {
    const SECRET: &str = "TOP_SECRET_VALUE";

    let error = LenientJsonDecoder::default()
        .decode::<PublicChoice>(&format!("\"{SECRET}\""))
        .expect_err("an unknown enum variant should fail deserialization");

    assert_eq!(error.privacy_policy(), ErrorPrivacyPolicy::Redacted);
    assert!(!error.message().contains(SECRET));
    assert!(!error.to_string().contains(SECRET));
    assert!(!format!("{error:?}").contains(SECRET));
    assert!(std::error::Error::source(&error).is_none());
    assert_eq!(error.normalized_line(), Some(1));
    assert_eq!(error.normalized_column(), Some(18));
}

#[test]
fn test_detailed_error_privacy_preserves_input_derived_serde_details() {
    const SECRET: &str = "TOP_SECRET_VALUE";

    let decoder = LenientJsonDecoder::new(
        JsonDecodeOptions::default()
            .with_error_privacy_policy(ErrorPrivacyPolicy::Detailed),
    );
    let error = decoder
        .decode::<PublicChoice>(&format!("\"{SECRET}\""))
        .expect_err("an unknown enum variant should fail deserialization");

    assert_eq!(error.privacy_policy(), ErrorPrivacyPolicy::Detailed);
    assert!(error.message().contains(SECRET));
    assert!(error.to_string().contains(SECRET));
    assert!(format!("{error:?}").contains(SECRET));
    let source = std::error::Error::source(&error)
        .expect("detailed errors should retain the serde_json source");
    assert!(source.to_string().contains(SECRET));
}

#[test]
fn test_default_invalid_json_error_does_not_expose_serde_source() {
    let error = LenientJsonDecoder::default()
        .decode_value("{")
        .expect_err("invalid JSON should fail parsing");

    assert_eq!(error.privacy_policy(), ErrorPrivacyPolicy::Redacted);
    assert!(std::error::Error::source(&error).is_none());
}

#[test]
fn test_normalization_errors_retain_the_configured_privacy_policy() {
    let redacted = LenientJsonDecoder::default()
        .decode_value("")
        .expect_err("empty input should fail normalization");
    let detailed = LenientJsonDecoder::new(
        JsonDecodeOptions::default()
            .with_error_privacy_policy(ErrorPrivacyPolicy::Detailed),
    )
    .decode_value("")
    .expect_err("empty input should fail normalization");

    assert_eq!(redacted.privacy_policy(), ErrorPrivacyPolicy::Redacted);
    assert_eq!(detailed.privacy_policy(), ErrorPrivacyPolicy::Detailed);
}

#[test]
fn test_error_display_for_deserialize_error_uses_context_message() {
    let error = LenientJsonDecoder::default()
        .decode::<u64>("\"text\"")
        .expect_err("string JSON should not deserialize into u64");
    assert_eq!(error.kind(), JsonDecodeErrorKind::Deserialize);
    assert_eq!(error.stage(), JsonDecodeStage::Deserialize);
    assert_eq!(error.raw_input_bytes(), 6);
    assert_eq!(error.normalized_input_bytes(), Some(6));
    assert_eq!(
        error.to_string(),
        "Failed to deserialize JSON value at normalized line 1 column 6"
    );
    assert!(std::error::Error::source(&error).is_none());
}

#[test]
fn test_error_partial_eq_compares_all_stable_fields() {
    let decoder = LenientJsonDecoder::default();
    let first = decoder
        .decode_value("{\n")
        .expect_err("invalid JSON should return parse error");
    let second = decoder
        .decode_value("{\n")
        .expect_err("invalid JSON should return parse error");
    assert_eq!(first, second);

    let third = decoder
        .decode_value("")
        .expect_err("empty input should return normalization error");
    assert_ne!(first, third);

    let detailed = LenientJsonDecoder::new(
        JsonDecodeOptions::default()
            .with_error_privacy_policy(ErrorPrivacyPolicy::Detailed),
    )
    .decode_value("")
    .expect_err("empty input should return normalization error");
    assert_ne!(third, detailed);
}
