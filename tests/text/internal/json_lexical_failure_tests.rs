// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests source coordinates retained for lexical failures.

use qubit_budget::json::JsonDecodeLimits;
use qubit_budget::json::JsonDecodeSession;
use qubit_json::text::JsonDecodeError;
use qubit_json::text::JsonTextDecoder;

/// Verifies lexical failures retain byte, line, and column coordinates.
#[test]
fn test_lexical_failure_reports_source_coordinates() {
    let mut session = JsonDecodeSession::owned(JsonDecodeLimits::empty());
    let error = JsonTextDecoder::new(&mut session)
        .validate(b"{\n\"key\" 1}")
        .expect_err("object member without colon must fail");

    let JsonDecodeError::Syntax(error) = error else {
        panic!("expected a syntax error");
    };
    assert_eq!((error.offset(), error.line(), error.column()), (8, 2, 7));
}
