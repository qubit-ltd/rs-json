// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests public mappings for lexical error reasons.

use qubit_budget::json::JsonDecodeLimits;
use qubit_budget::json::JsonDecodeSession;
use qubit_json::text::JsonDecodeError;
use qubit_json::text::JsonSyntaxErrorReason;
use qubit_json::text::JsonTextDecoder;

/// Verifies trailing bytes retain their stable lexical rejection reason.
#[test]
fn test_lexical_error_reason_reports_trailing_characters() {
    let mut session = JsonDecodeSession::owned(JsonDecodeLimits::empty());
    let error = JsonTextDecoder::new(&mut session)
        .validate(b"true false")
        .expect_err("trailing JSON value must be rejected");

    let JsonDecodeError::Syntax(error) = error else {
        panic!("expected a syntax error");
    };
    assert_eq!(error.reason(), JsonSyntaxErrorReason::TrailingCharacters);
}
