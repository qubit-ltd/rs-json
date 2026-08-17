// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests size-limit discriminator behavior through public error diagnostics.

use qubit_json::decode::NormalizingJsonDecodeOptions;
use qubit_json::decode::NormalizingJsonDecoder;

/// Verifies that raw-size failures expose only the raw limit.
///
/// # Panics
///
/// Panics when the public raw-size diagnostics are not observed.
#[test]
fn test_raw_size_limit_exposes_only_raw_limit() {
    let decoder = NormalizingJsonDecoder::new(
        NormalizingJsonDecodeOptions::builder()
            .max_input_bytes(Some(7))
            .build(),
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
    assert_eq!(error.max_normalized_bytes(), None);
}

/// Verifies that normalized-size failures expose only the normalized limit.
///
/// # Panics
///
/// Panics when the public normalized-size diagnostics are not observed.
#[test]
fn test_normalized_size_limit_exposes_only_normalized_limit() {
    let decoder = NormalizingJsonDecoder::new(
        NormalizingJsonDecodeOptions::builder()
            .max_normalized_bytes(Some(7))
            .build(),
    );
    let error = decoder.decode_utf8::<String>("\"\u{0000}\"").expect_err(
        "oversized normalized input should return an input-too-large error",
    );
    assert_eq!(
        error.to_string(),
        "Normalized JSON input is too large: 8 bytes exceed configured limit 7 bytes"
    );
    assert_eq!(error.raw_input_bytes(), 3);
    assert_eq!(error.normalized_input_bytes(), Some(8));
    assert_eq!(error.max_input_bytes(), None);
    assert_eq!(error.max_normalized_bytes(), Some(7));
}
