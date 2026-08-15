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
use qubit_json::text::JsonDecodeError;
use qubit_json::text::JsonTextDecoder;

/// Verifies malformed lexical input becomes a strict syntax error.
#[test]
fn test_lexical_error_maps_to_syntax_error() {
    let mut session = JsonDecodeSession::owned(JsonDecodeLimits::empty());
    let error = JsonTextDecoder::new(&mut session)
        .decode::<serde_json::Value>(b"[")
        .expect_err("unterminated array must fail lexical admission");

    assert!(matches!(error, JsonDecodeError::Syntax(_)));
}
