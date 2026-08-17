// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
// =============================================================================
//! Verifies the new strict and normalizing decoder entry points.

use qubit_budget::json::JsonDecodeLimits;
use qubit_budget::json::JsonDecodeSession;
use qubit_json::decode::JsonDecoder;
use qubit_json::decode::NormalizingJsonDecoder;

/// Creates a reusable session with no configured resource limits.
fn session() -> JsonDecodeSession<'static> {
    JsonDecodeSession::owned(JsonDecodeLimits::builder().build())
}

/// Verifies that strict string decoding can borrow from the input.
#[test]
fn test_json_decoder_decode_str_borrows_input() {
    let mut decoder = JsonDecoder::default();
    let mut session = session();
    let input = "\"borrowed\"";

    let value: &str = decoder
        .decode_str(input)
        .expect("strict string decoding should succeed");

    assert_eq!(value, "borrowed");
}

/// Verifies that strict UTF-8 byte decoding can borrow from the input.
#[test]
fn test_json_decoder_decode_utf8_borrows_input() {
    let mut decoder = JsonDecoder::default();
    let mut session = session();
    let input = br#""borrowed""#;

    let value: &str = decoder
        .decode_utf8(input)
        .expect("strict UTF-8 decoding should succeed");

    assert_eq!(value, "borrowed");
}

/// Verifies that strict validation accepts string and UTF-8 entry points.
#[test]
fn test_json_decoder_validation_entry_points() {
    let mut decoder = JsonDecoder::default();
    let mut session = session();

    decoder
        .validate_str("null")
        .expect("strict string validation should succeed");
    decoder
        .validate_utf8(b"true")
        .expect("strict UTF-8 validation should succeed");
}

/// Verifies that normalizing string decoding returns an owned target.
#[test]
fn test_normalizing_decoder_decode_str_returns_owned_value() {
    let mut decoder = NormalizingJsonDecoder::default();
    let mut session = session();

    let value: String = decoder
        .decode_str("  \"owned\"  ")
        .expect("normalizing string decoding should succeed");

    assert_eq!(value, "owned");
}

/// Verifies that normalizing UTF-8 byte decoding returns an owned target.
#[test]
fn test_normalizing_decoder_decode_utf8_returns_owned_value() {
    let mut decoder = NormalizingJsonDecoder::default();
    let mut session = session();

    let value: String = decoder
        .decode_utf8(br#"  "owned"  "#)
        .expect("normalizing UTF-8 decoding should succeed");

    assert_eq!(value, "owned");
}
