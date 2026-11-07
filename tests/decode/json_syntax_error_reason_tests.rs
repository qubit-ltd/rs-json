// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests stable strict JSON syntax reason formatting.

use qubit_json::decode::JsonSyntaxErrorReason;

/// Verifies syntax reasons retain their privacy-safe display text.
#[test]
fn test_json_syntax_error_reason_formats_unexpected_byte() {
    assert_eq!(
        JsonSyntaxErrorReason::UnexpectedByte.to_string(),
        "unexpected byte",
    );
}

/// Verifies every public syntax reason has a stable display representation.
#[test]
fn test_json_syntax_error_reason_formats_every_variant() {
    let reasons_and_text = [
        (
            JsonSyntaxErrorReason::UnexpectedEnd,
            "unexpected end of input",
        ),
        (JsonSyntaxErrorReason::UnexpectedByte, "unexpected byte"),
        (JsonSyntaxErrorReason::ExpectedColon, "expected ':'"),
        (
            JsonSyntaxErrorReason::ExpectedCommaOrArrayEnd,
            "expected ',' or ']' in array",
        ),
        (
            JsonSyntaxErrorReason::ExpectedCommaOrObjectEnd,
            "expected ',' or '}' in object",
        ),
        (
            JsonSyntaxErrorReason::ExpectedObjectKey,
            "expected object key",
        ),
        (
            JsonSyntaxErrorReason::InvalidEscape,
            "invalid string escape",
        ),
        (
            JsonSyntaxErrorReason::InvalidUnicodeEscape,
            "invalid Unicode escape",
        ),
        (
            JsonSyntaxErrorReason::UnpairedSurrogate,
            "unpaired Unicode surrogate",
        ),
        (JsonSyntaxErrorReason::InvalidUtf8, "invalid UTF-8"),
        (JsonSyntaxErrorReason::InvalidNumber, "invalid JSON number"),
        (
            JsonSyntaxErrorReason::IntegerOutOfRange,
            "JSON integer is outside the supported 64-bit range",
        ),
        (
            JsonSyntaxErrorReason::FloatOutOfRange,
            "JSON number is outside the finite f64 range",
        ),
        (
            JsonSyntaxErrorReason::TrailingCharacters,
            "trailing characters",
        ),
        (
            JsonSyntaxErrorReason::NestingOverflow,
            "JSON nesting overflow",
        ),
    ];

    for (reason, expected_text) in reasons_and_text {
        assert_eq!(reason.to_string(), expected_text);
    }
}
