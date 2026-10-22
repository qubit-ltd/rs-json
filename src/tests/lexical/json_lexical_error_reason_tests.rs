// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests stable display text for crate-private lexical rejection reasons.

use crate::lexical::JsonLexicalErrorReason;

/// Verifies every stable lexical rejection reason has a deterministic
/// human-readable representation.
#[test]
fn test_json_lexical_error_reason_formats_every_variant() {
    let cases = [
        (JsonLexicalErrorReason::UnexpectedEnd, "unexpected end of input"),
        (JsonLexicalErrorReason::UnexpectedByte, "unexpected byte"),
        (JsonLexicalErrorReason::ExpectedColon, "expected ':'"),
        (
            JsonLexicalErrorReason::ExpectedCommaOrArrayEnd,
            "expected ',' or ']' in array",
        ),
        (
            JsonLexicalErrorReason::ExpectedCommaOrObjectEnd,
            "expected ',' or '}' in object",
        ),
        (JsonLexicalErrorReason::ExpectedObjectKey, "expected object key"),
        (JsonLexicalErrorReason::InvalidEscape, "invalid string escape"),
        (JsonLexicalErrorReason::InvalidUnicodeEscape, "invalid Unicode escape"),
        (JsonLexicalErrorReason::UnpairedSurrogate, "unpaired Unicode surrogate"),
        (JsonLexicalErrorReason::InvalidUtf8, "invalid UTF-8"),
        (JsonLexicalErrorReason::InvalidNumber, "invalid JSON number"),
        (
            JsonLexicalErrorReason::IntegerOutOfRange,
            "JSON integer is outside the supported 64-bit range",
        ),
        (
            JsonLexicalErrorReason::FloatOutOfRange,
            "JSON number is outside the finite f64 range",
        ),
        (JsonLexicalErrorReason::TrailingCharacters, "trailing characters"),
        (JsonLexicalErrorReason::NestingOverflow, "JSON nesting overflow"),
    ];

    for (reason, expected) in cases {
        assert_eq!(reason.to_string(), expected);
    }
}
