// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests byte-cursor behavior through strict decoder input handling.

use qubit_budget::json::JsonDecodeLimits;
use qubit_budget::json::JsonDecodeSession;
use qubit_budget::json::JsonResource;
use qubit_json::decode::JsonDecodeErrorKind;
use qubit_json::decode::JsonDecoder;
use qubit_json::decode::JsonSyntaxErrorReason;

/// Verifies the lexical cursor skips JSON whitespace before and after a value.
#[test]
fn test_cursor_skips_json_whitespace() {
    let session = JsonDecodeSession::from_limits(JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::builder().build());
    let value = JsonDecoder::new(session)
        .decode_utf8::<u8>(b" \n\t 7\r ")
        .expect("whitespace-wrapped JSON number should decode");

    assert_eq!(value, 7);
}

/// Verifies that malformed scalar and compound inputs reach stable lexical
/// classifications through the public decoder boundary.
#[test]
fn test_cursor_reports_scalar_and_container_syntax_errors() {
    let cases: &[(&[u8], JsonSyntaxErrorReason)] = &[
        (b"", JsonSyntaxErrorReason::UnexpectedEnd),
        (b"@", JsonSyntaxErrorReason::UnexpectedByte { byte: b'@' }),
        (b"truex", JsonSyntaxErrorReason::UnexpectedByte { byte: b'x' }),
        (b"true false", JsonSyntaxErrorReason::TrailingCharacters),
        (b"[1 2]", JsonSyntaxErrorReason::ExpectedCommaOrArrayEnd),
        (b"[1,]", JsonSyntaxErrorReason::UnexpectedByte { byte: b']' }),
        (b"[1", JsonSyntaxErrorReason::UnexpectedEnd),
        (b"{1:2}", JsonSyntaxErrorReason::ExpectedObjectKey),
        (br#"{"a" 1}"#, JsonSyntaxErrorReason::ExpectedColon),
        (br#"{"a":1 "b":2}"#, JsonSyntaxErrorReason::ExpectedCommaOrObjectEnd),
        (br#"{"a":1,}"#, JsonSyntaxErrorReason::UnexpectedByte { byte: b'}' }),
        (br#"{"a":1"#, JsonSyntaxErrorReason::UnexpectedEnd),
        (br#""\q""#, JsonSyntaxErrorReason::InvalidEscape),
        (br#""\u12"#, JsonSyntaxErrorReason::UnexpectedEnd),
        (br#""\u12x4""#, JsonSyntaxErrorReason::InvalidUnicodeEscape),
        (br#""\uD800"#, JsonSyntaxErrorReason::UnexpectedEnd),
        (br#""\uDC00""#, JsonSyntaxErrorReason::UnpairedSurrogate),
        (b"01", JsonSyntaxErrorReason::InvalidNumber),
        (b"1.", JsonSyntaxErrorReason::InvalidNumber),
        (b"1e+", JsonSyntaxErrorReason::InvalidNumber),
    ];

    for (input, expected) in cases {
        let session = JsonDecodeSession::from_limits(JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::builder().build());
        let error = JsonDecoder::new(session)
            .decode_utf8::<serde_json::Value>(input)
            .expect_err("malformed input should be rejected");
        let error = error.syntax_error().expect("expected a syntax error");
        assert_eq!(error.reason(), *expected, "input: {input:?}");
    }
}

/// Verifies malformed literals identify the first byte after a complete token.
#[test]
fn test_cursor_reports_the_invalid_literal_delimiter_location() {
    let session = JsonDecodeSession::from_limits(JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::builder().build());
    let error = JsonDecoder::new(session)
        .decode_utf8::<serde_json::Value>(b"truex")
        .expect_err("a literal must end at a JSON value delimiter");
    let error = error.syntax_error().expect("expected a syntax error");

    assert_eq!(error.reason(), JsonSyntaxErrorReason::UnexpectedByte { byte: b'x' });
    assert_eq!(error.line(), 1);
    assert_eq!(error.column(), 5);
}

/// Verifies UTF-8 width handling, Unicode escapes, and source coordinates.
#[test]
fn test_cursor_accepts_unicode_and_reports_coordinates() {
    let session = JsonDecodeSession::from_limits(JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::builder().build());
    let value = JsonDecoder::new(session)
        .decode_utf8::<String>("\"é\\uD83D\\uDE00\"".as_bytes())
        .expect("valid UTF-8 and surrogate-pair escapes should decode");
    assert_eq!(value, "é😀");

    let session = JsonDecodeSession::from_limits(JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::builder().build());
    let error = JsonDecoder::new(session)
        .decode_utf8::<serde_json::Value>("1\r\n@".as_bytes())
        .expect_err("invalid byte should be rejected");
    let error = error.syntax_error().expect("expected a syntax error");
    assert_eq!(error.line(), 2);
    assert_eq!(error.column(), 1);
}

/// Verifies invalid UTF-8 in a JSON string is classified without panicking.
#[test]
fn test_cursor_rejects_invalid_utf8_inside_string() {
    let session = JsonDecodeSession::from_limits(JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::builder().build());
    let error = JsonDecoder::new(session)
        .decode_utf8::<serde_json::Value>(b"\"\x80\"")
        .expect_err("invalid UTF-8 should be rejected");
    assert_eq!(error.kind(), JsonDecodeErrorKind::InvalidUtf8);
    assert_eq!(error.utf8_valid_up_to(), Some(1));
}
