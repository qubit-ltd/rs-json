// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests crate-private number range contracts used during lexical admission.

#![cfg(feature = "jcs")]

use qubit_budget::json::JsonDecodeLimits;
use qubit_budget::json::JsonDecodeSession;
use qubit_budget::json::JsonResource;
use qubit_budget::json::JsonValueLimits;

use crate::lexical::JsonLexicalError;
use crate::lexical::JsonLexicalScanner;
use crate::lexical::JsonNumberModel;

/// Checks whether the scanner admits one input under the selected number
/// model and limits.
fn scan(input: &[u8], model: JsonNumberModel, limits: JsonDecodeLimits<JsonResource, usize>) -> bool {
    let mut session = JsonDecodeSession::from_limits(limits);
    let mut attempt = session.begin_value();
    JsonLexicalScanner::new(attempt.value_transaction_mut(), true)
        .with_number_model(model)
        .scan(input)
        .is_ok()
}

#[test]
fn test_jcs_number_model_accepts_large_integer_only_in_binary64() {
    let limits = JsonDecodeLimits::<JsonResource, usize>::builder()
        .value_limits(JsonValueLimits::builder().build())
        .build();
    assert!(
        scan(b"100000000000000000000", JsonNumberModel::Binary64, limits),
        "binary64 mode must admit an integer outside the i64/u64 range"
    );
    assert!(
        !scan(b"100000000000000000000", JsonNumberModel::Integer64, limits),
        "integer64 mode must reject an integer outside the i64/u64 range"
    );
}

#[test]
fn test_jcs_number_model_rejects_non_finite_binary64() {
    let limits = JsonDecodeLimits::<JsonResource, usize>::builder()
        .value_limits(JsonValueLimits::builder().build())
        .build();
    assert!(
        !scan(b"1e400", JsonNumberModel::Binary64, limits),
        "binary64 mode must reject a non-finite numeric value"
    );
}

#[test]
fn test_jcs_number_model_checks_number_bytes_before_range() {
    let limits = JsonDecodeLimits::<JsonResource, usize>::builder()
        .value_limits(JsonValueLimits::builder().max_number_bytes(2).build())
        .build();
    let mut session = JsonDecodeSession::from_limits(limits);
    let mut attempt = session.begin_value();
    let error = JsonLexicalScanner::new(attempt.value_transaction_mut(), true)
        .with_number_model(JsonNumberModel::Binary64)
        .scan(b"1e400")
        .expect_err("token length budget rejects before range parsing");
    assert!(
        matches!(error, JsonLexicalError::Budget(_)),
        "the number-byte budget must reject the token before range parsing"
    );
}
