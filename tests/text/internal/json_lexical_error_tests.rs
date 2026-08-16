// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests lexical rejection conversion through the strict decoder.

use qubit_budget::json::JsonDecodeLimits;
use qubit_budget::json::JsonDecodeSession;
use qubit_budget::json::JsonResource;
use qubit_json::decode::JsonDecodeError;
use qubit_json::decode::JsonDecoder;

/// Verifies malformed lexical input becomes a strict syntax error.
#[test]
fn test_lexical_error_maps_to_syntax_error() {
    let mut session = JsonDecodeSession::owned(
        JsonDecodeLimits::<JsonResource, usize>::builder().build(),
    );
    let error = JsonDecoder::new(&mut session)
        .decode::<serde_json::Value>(b"[")
        .expect_err("unterminated array must fail lexical admission");

    assert!(matches!(error, JsonDecodeError::Syntax(_)));
}
