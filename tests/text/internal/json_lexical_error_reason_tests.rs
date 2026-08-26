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
use qubit_budget::json::JsonResource;
use qubit_json::decode::JsonDecoder;
use qubit_json::decode::JsonSyntaxErrorReason;

/// Verifies trailing bytes retain their stable lexical rejection reason.
#[test]
fn test_lexical_error_reason_reports_trailing_characters() {
    let session = JsonDecodeSession::owned(JsonDecodeLimits::<JsonResource, usize>::builder().build());
    let error = JsonDecoder::new(session)
        .validate_utf8(b"true false")
        .expect_err("trailing JSON value must be rejected");

    let error = error.syntax_error().expect("expected a syntax error");
    assert_eq!(error.reason(), JsonSyntaxErrorReason::TrailingCharacters);
}
