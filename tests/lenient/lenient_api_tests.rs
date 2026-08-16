// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests for the public lenient JSON domain API.

use std::error::Error;

use qubit_json::decode::DiagnosticPolicy;
use qubit_json::decode::NormalizingJsonDecodeErrorKind;
use qubit_json::decode::NormalizingJsonDecodeOptions;
use qubit_json::decode::NormalizingJsonDecodeStage;
use qubit_json::decode::NormalizingJsonDecoder;

/// Verifies that lenient decoding exposes domain-owned types and preserves
/// redacted error privacy.
///
/// Panics when the public API or privacy contract is not satisfied.
#[test]
fn test_lenient_domain_owns_its_public_types() {
    let decoder = NormalizingJsonDecoder::new(
        NormalizingJsonDecodeOptions::builder()
            .diagnostic_policy(DiagnosticPolicy::Redacted)
            .build(),
    );
    let error = decoder
        .decode::<u64>(r#""TOP_SECRET""#)
        .expect_err("a string cannot deserialize into u64");

    assert_eq!(error.kind(), NormalizingJsonDecodeErrorKind::Deserialize);
    assert_eq!(error.stage(), NormalizingJsonDecodeStage::Deserialize);
    assert_eq!(error.privacy_policy(), DiagnosticPolicy::Redacted);
    assert!(!error.to_string().contains("TOP_SECRET"));
    assert!(error.source().is_none());
}
