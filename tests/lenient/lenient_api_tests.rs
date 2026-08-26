// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests for the public lenient JSON domain API.

use std::error::Error;

use qubit_budget::json::JsonDecodeLimits;
use qubit_json::decode::DiagnosticPolicy;
use qubit_json::decode::JsonDecodeErrorKind;
use qubit_json::decode::JsonDecodeStage;
use qubit_json::decode::NormalizingJsonDecodePolicy;
use qubit_json::decode::NormalizingJsonDecoder;

/// Verifies that lenient decoding exposes domain-owned types and preserves
/// redacted error privacy.
///
/// Panics when the public API or privacy contract is not satisfied.
#[test]
fn test_lenient_domain_owns_its_public_types() {
    let mut decoder = NormalizingJsonDecoder::owned(
        NormalizingJsonDecodePolicy::builder()
            .diagnostic_policy(DiagnosticPolicy::Redacted)
            .build(),
        JsonDecodeLimits::default(),
    );
    let error = decoder
        .decode_str::<u64>(r#""TOP_SECRET""#)
        .expect_err("a string cannot deserialize into u64");

    assert_eq!(error.kind(), JsonDecodeErrorKind::Deserialize);
    assert_eq!(error.stage(), JsonDecodeStage::Deserialize);
    assert_eq!(error.diagnostic_policy(), DiagnosticPolicy::Redacted);
    assert!(!error.to_string().contains("TOP_SECRET"));
    assert!(error.source().is_none());
}
