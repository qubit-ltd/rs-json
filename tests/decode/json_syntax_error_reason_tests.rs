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
    assert_eq!(JsonSyntaxErrorReason::UnexpectedByte.to_string(), "unexpected byte",);
}

/// Verifies every public syntax reason has a stable display representation.
#[test]
fn test_json_syntax_error_reason_formats_every_variant() {
    let reasons = [
        JsonSyntaxErrorReason::UnexpectedEnd,
        JsonSyntaxErrorReason::UnexpectedByte,
        JsonSyntaxErrorReason::ExpectedColon,
        JsonSyntaxErrorReason::ExpectedCommaOrArrayEnd,
        JsonSyntaxErrorReason::ExpectedCommaOrObjectEnd,
        JsonSyntaxErrorReason::ExpectedObjectKey,
        JsonSyntaxErrorReason::InvalidEscape,
        JsonSyntaxErrorReason::InvalidUnicodeEscape,
        JsonSyntaxErrorReason::UnpairedSurrogate,
        JsonSyntaxErrorReason::InvalidUtf8,
        JsonSyntaxErrorReason::InvalidNumber,
        JsonSyntaxErrorReason::IntegerOutOfRange,
        JsonSyntaxErrorReason::FloatOutOfRange,
        JsonSyntaxErrorReason::TrailingCharacters,
        JsonSyntaxErrorReason::NestingOverflow,
    ];

    for reason in reasons {
        assert!(!reason.to_string().is_empty());
    }
}
