// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests for the public lenient JSON domain API.

use std::error::Error;

use qubit_json::lenient::ErrorPrivacyPolicy;
use qubit_json::lenient::LenientJsonDecodeErrorKind;
use qubit_json::lenient::LenientJsonDecodeOptions;
use qubit_json::lenient::LenientJsonDecodeStage;
use qubit_json::lenient::LenientJsonDecoder;

/// Verifies that lenient decoding exposes domain-owned types and preserves
/// redacted error privacy.
///
/// Panics when the public API or privacy contract is not satisfied.
#[test]
fn test_lenient_domain_owns_its_public_types() {
    let decoder = LenientJsonDecoder::new(
        LenientJsonDecodeOptions::strict()
            .with_error_privacy_policy(ErrorPrivacyPolicy::Redacted),
    );
    let error = decoder
        .decode::<u64>(r#""TOP_SECRET""#)
        .expect_err("a string cannot deserialize into u64");

    assert_eq!(error.kind(), LenientJsonDecodeErrorKind::Deserialize);
    assert_eq!(error.stage(), LenientJsonDecodeStage::Deserialize);
    assert_eq!(error.privacy_policy(), ErrorPrivacyPolicy::Redacted);
    assert!(!error.to_string().contains("TOP_SECRET"));
    assert!(error.source().is_none());
}
