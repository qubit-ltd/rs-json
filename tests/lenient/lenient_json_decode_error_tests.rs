// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests for the public [`qubit_json::lenient::LenientJsonDecodeError`] type.

use qubit_budget::ResourceLimit;
use qubit_budget::json::JsonDecodeLimits;
use qubit_budget::json::JsonDecodeSession;
use qubit_budget::json::JsonResource;
use qubit_budget::json::JsonValueLimits;
use qubit_json::lenient::ErrorPrivacyPolicy;
use qubit_json::lenient::JsonTopLevelKind;
use qubit_json::lenient::LenientJsonDecodeErrorKind;
use qubit_json::lenient::LenientJsonDecodeOptions;
use qubit_json::lenient::LenientJsonDecodeStage;
use qubit_json::lenient::LenientJsonDecoder;
use serde_json::Value;

use crate::fixtures::PublicChoice;

/// Verifies that budget errors retain their structured rejection details.
///
/// # Panics
///
/// Panics when admission does not reject the value or the public error omits
/// its budget resource.
#[test]
fn test_budget_error_exposes_measured_rejection_details() {
    let limits = JsonDecodeLimits::empty().with_value_limits(
        JsonValueLimits::empty()
            .with_string_bytes_limit(ResourceLimit::new(JsonResource::StringBytes, 0)),
    );
    let mut session = JsonDecodeSession::owned(limits);

    let error = LenientJsonDecoder::default()
        .decode_with_session::<Value>(r#"{"k":"v"}"#, &mut session)
        .expect_err("string budget must reject the normalized value");

    assert_eq!(error.kind(), LenientJsonDecodeErrorKind::Budget);
    assert_eq!(error.stage(), LenientJsonDecodeStage::Admission);
    assert_eq!(
        *error
            .measured_budget_error()
            .expect("budget details must be retained")
            .resource(),
        JsonResource::StringBytes,
    );
    assert!(std::error::Error::source(&error).is_some());
}

/// Verifies that error display for empty input uses message.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_error_display_for_empty_input_uses_message() {
    let error = LenientJsonDecoder::default()
        .decode_value("")
        .expect_err("empty input should return a normalization error");
    assert_eq!(error.to_string(), "JSON input is empty after normalization");
    assert_eq!(error.privacy_policy(), ErrorPrivacyPolicy::Redacted);
    assert_eq!(error.raw_input_bytes(), 0);
    assert_eq!(error.normalized_input_bytes(), None);
    assert_eq!(error.utf8_valid_up_to(), None);
    assert_eq!(error.utf8_error_len(), None);
    assert!(std::error::Error::source(&error).is_none());
}

/// Verifies that error exposes top level mismatch context.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_error_exposes_top_level_mismatch_context() {
    let error = LenientJsonDecoder::default()
        .decode_object::<Value>("[]")
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

/// Verifies that error exposes immutable normalized diagnostics without
/// duplicate location.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_error_exposes_immutable_normalized_diagnostics_without_duplicate_location() {
    let error = LenientJsonDecoder::default()
        .decode_value("  {\n")
        .expect_err("incomplete JSON should fail after whitespace normalization");

    assert_eq!(error.kind(), LenientJsonDecodeErrorKind::InvalidJson);
    assert_eq!(error.stage(), LenientJsonDecodeStage::Parse);
    assert_eq!(error.raw_input_bytes(), 4);
    assert_eq!(error.normalized_input_bytes(), Some(1));
    assert_eq!(error.normalized_line(), Some(1));
    assert_eq!(error.normalized_column(), Some(1));
    assert_eq!(
        error.to_string(),
        "Failed to parse JSON at normalized line 1 column 1"
    );
    assert!(std::error::Error::source(&error).is_none());
}

/// Verifies that error source for invalid json preserves serde error.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_error_source_for_invalid_json_preserves_serde_error() {
    let decoder = LenientJsonDecoder::new(
        LenientJsonDecodeOptions::default().with_error_privacy_policy(ErrorPrivacyPolicy::Detailed),
    );
    let error = decoder
        .decode_value("{")
        .expect_err("invalid JSON should preserve the parser source error");
    let source = std::error::Error::source(&error)
        .expect("invalid JSON errors should expose the serde_json source");
    assert!(source.to_string().contains("EOF"));
}

/// Verifies that default error privacy redacts input derived serde details.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_default_error_privacy_redacts_input_derived_serde_details() {
    const SECRET: &str = "TOP_SECRET_VALUE";

    let error = LenientJsonDecoder::default()
        .decode::<PublicChoice>(&format!("\"{SECRET}\""))
        .expect_err("an unknown enum variant should fail deserialization");

    assert_eq!(error.privacy_policy(), ErrorPrivacyPolicy::Redacted);
    assert!(!error.to_string().contains(SECRET));
    assert!(!format!("{error:?}").contains(SECRET));
    assert!(std::error::Error::source(&error).is_none());
    assert_eq!(error.normalized_line(), Some(1));
    assert_eq!(error.normalized_column(), Some(18));
}

/// Verifies that detailed error privacy preserves input derived serde details.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_detailed_error_privacy_preserves_input_derived_serde_details() {
    const SECRET: &str = "TOP_SECRET_VALUE";

    let decoder = LenientJsonDecoder::new(
        LenientJsonDecodeOptions::default().with_error_privacy_policy(ErrorPrivacyPolicy::Detailed),
    );
    let error = decoder
        .decode::<PublicChoice>(&format!("\"{SECRET}\""))
        .expect_err("an unknown enum variant should fail deserialization");

    assert_eq!(error.privacy_policy(), ErrorPrivacyPolicy::Detailed);
    assert!(error.to_string().contains(SECRET));
    assert!(format!("{error:?}").contains(SECRET));
    let source = std::error::Error::source(&error)
        .expect("detailed errors should retain the serde_json source");
    assert!(source.to_string().contains(SECRET));
}

/// Verifies that default invalid json error does not expose serde source.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_default_invalid_json_error_does_not_expose_serde_source() {
    let error = LenientJsonDecoder::default()
        .decode_value("{")
        .expect_err("invalid JSON should fail parsing");

    assert_eq!(error.privacy_policy(), ErrorPrivacyPolicy::Redacted);
    assert!(std::error::Error::source(&error).is_none());
}

/// Verifies that invalid utf8 redacted error does not expose its source.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_invalid_utf8_redacted_error_does_not_expose_source() {
    let error = LenientJsonDecoder::default()
        .decode_slice::<Value>(&[0xff])
        .expect_err("invalid UTF-8 must fail");
    assert_eq!(error.privacy_policy(), ErrorPrivacyPolicy::Redacted);
    assert_eq!(error.utf8_valid_up_to(), Some(0));
    assert_eq!(error.utf8_error_len(), Some(1));
    assert!(std::error::Error::source(&error).is_none());
    assert!(!format!("{error:?}").contains("255"));
}

/// Verifies that invalid UTF-8 exposes safe byte-position diagnostics.
///
/// # Panics
///
/// Panics when the decoder omits the valid prefix or invalid sequence length.
#[test]
fn test_invalid_utf8_exposes_safe_position_diagnostics() {
    let definite = LenientJsonDecoder::default()
        .decode_slice::<Value>(&[b'{', 0xff])
        .expect_err("invalid UTF-8 must fail");
    assert_eq!(definite.utf8_valid_up_to(), Some(1));
    assert_eq!(definite.utf8_error_len(), Some(1));

    let incomplete = LenientJsonDecoder::default()
        .decode_slice::<Value>(&[0xe2, 0x82])
        .expect_err("incomplete UTF-8 must fail");
    assert_eq!(incomplete.utf8_valid_up_to(), Some(0));
    assert_eq!(incomplete.utf8_error_len(), None);
}

/// Verifies that invalid utf8 detailed error retains utf8 source.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_invalid_utf8_detailed_error_retains_utf8_source() {
    let decoder = LenientJsonDecoder::new(
        LenientJsonDecodeOptions::strict().with_error_privacy_policy(ErrorPrivacyPolicy::Detailed),
    );
    let error = decoder
        .decode_slice::<Value>(&[0xff])
        .expect_err("invalid UTF-8 must fail");
    assert_eq!(error.utf8_valid_up_to(), Some(0));
    assert_eq!(error.utf8_error_len(), Some(1));
    let source = std::error::Error::source(&error).expect("detailed errors must retain Utf8Error");
    assert!(source.downcast_ref::<std::str::Utf8Error>().is_some());
}

/// Verifies that normalization errors retain the configured privacy policy.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_normalization_errors_retain_the_configured_privacy_policy() {
    let redacted = LenientJsonDecoder::default()
        .decode_value("")
        .expect_err("empty input should fail normalization");
    let detailed = LenientJsonDecoder::new(
        LenientJsonDecodeOptions::default().with_error_privacy_policy(ErrorPrivacyPolicy::Detailed),
    )
    .decode_value("")
    .expect_err("empty input should fail normalization");

    assert_eq!(redacted.privacy_policy(), ErrorPrivacyPolicy::Redacted);
    assert_eq!(detailed.privacy_policy(), ErrorPrivacyPolicy::Detailed);
}

/// Verifies that error display for deserialize error uses context message.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_error_display_for_deserialize_error_uses_context_message() {
    let error = LenientJsonDecoder::default()
        .decode::<u64>("\"text\"")
        .expect_err("string JSON should not deserialize into u64");
    assert_eq!(error.kind(), LenientJsonDecodeErrorKind::Deserialize);
    assert_eq!(error.stage(), LenientJsonDecodeStage::Deserialize);
    assert_eq!(error.raw_input_bytes(), 6);
    assert_eq!(error.normalized_input_bytes(), Some(6));
    assert_eq!(
        error.to_string(),
        "Failed to deserialize JSON value at normalized line 1 column 6"
    );
    assert!(std::error::Error::source(&error).is_none());
}

/// Verifies that error partial eq compares all stable fields.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
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
        LenientJsonDecodeOptions::default().with_error_privacy_policy(ErrorPrivacyPolicy::Detailed),
    )
    .decode_value("")
    .expect_err("empty input should return normalization error");
    assert_ne!(third, detailed);

    let invalid_utf8_at_start = decoder
        .decode_slice::<Value>(&[0xff])
        .expect_err("invalid UTF-8 should return a decode error");
    let invalid_utf8_after_prefix = decoder
        .decode_slice::<Value>(&[b'a', 0xff])
        .expect_err("invalid UTF-8 should return a decode error");
    assert_ne!(invalid_utf8_at_start, invalid_utf8_after_prefix);

    assert_eq!(first.clone(), first);
}
