// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests size-limit discriminator behavior through public error diagnostics.

use qubit_budget::json::JsonDecodeLimits;
use qubit_json::decode::JsonDecodeError;
use qubit_json::decode::JsonDecodeErrorKind;
use qubit_json::decode::JsonDecodeStage;
use qubit_json::decode::NormalizingJsonDecodePolicy;
use qubit_json::decode::NormalizingJsonDecoder;

/// Returns the configured limit retained by a measured budget failure.
fn configured_limit(error: &JsonDecodeError) -> usize {
    error
        .budget_error()
        .and_then(|error| error.budget_error())
        .expect("the input-size failure must contain a budget error")
        .configured_limit()
}

/// Verifies that raw-size failures expose only the raw limit.
///
/// # Panics
///
/// Panics when the public raw-size diagnostics are not observed.
#[test]
fn test_raw_size_limit_exposes_only_raw_limit() {
    let mut decoder = NormalizingJsonDecoder::with_limits(
        NormalizingJsonDecodePolicy::default(),
        JsonDecodeLimits::builder().max_input_bytes(7).build(),
    );
    let error = decoder
        .decode_value("{\"a\": 1}")
        .expect_err("oversized input should return an input-too-large error");
    assert_eq!(error.kind(), JsonDecodeErrorKind::Budget);
    assert_eq!(error.stage(), JsonDecodeStage::Input);
    assert_eq!(error.raw_input_bytes(), 8);
    assert_eq!(error.normalized_input_bytes(), None);
    assert_eq!(configured_limit(&error), 7);
}

/// Verifies that normalized-size failures expose only the normalized limit.
///
/// # Panics
///
/// Panics when the public normalized-size diagnostics are not observed.
#[test]
fn test_normalized_size_limit_exposes_only_normalized_limit() {
    let mut decoder = NormalizingJsonDecoder::with_limits(
        NormalizingJsonDecodePolicy::default(),
        JsonDecodeLimits::builder().max_normalized_input_bytes(7).build(),
    );
    let error = decoder
        .decode_str::<String>("\"\u{0000}\"")
        .expect_err("oversized normalized input should return an input-too-large error");
    assert_eq!(error.kind(), JsonDecodeErrorKind::Budget);
    assert_eq!(error.stage(), JsonDecodeStage::Normalize);
    assert_eq!(error.raw_input_bytes(), 3);
    assert_eq!(error.normalized_input_bytes(), Some(8));
    assert_eq!(configured_limit(&error), 7);
}
