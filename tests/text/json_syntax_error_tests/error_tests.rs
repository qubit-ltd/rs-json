// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests structured JSON syntax errors.

use qubit_json::text::JsonSyntaxError;
use qubit_json::text::JsonSyntaxErrorReason;

#[test]
fn test_json_syntax_error_exposes_location_and_reason() {
    let error = JsonSyntaxError::new(7, 2, 3, JsonSyntaxErrorReason::ExpectedColon);
    assert_eq!(error.offset(), 7);
    assert_eq!(error.line(), 2);
    assert_eq!(error.column(), 3);
    assert_eq!(error.reason(), JsonSyntaxErrorReason::ExpectedColon);
    assert_eq!(
        error.to_string(),
        "expected ':' at line 2 column 3 (byte offset 7)",
    );
}
