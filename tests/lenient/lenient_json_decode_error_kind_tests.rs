// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests for the public `NormalizingJsonDecodeErrorKind` type.

use std::str::FromStr;

use qubit_json::decode::NormalizingJsonDecodeErrorKind;

/// Verifies that decode error kind display uses snake case names.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_error_kind_display_uses_snake_case_names() {
    assert_eq!(
        NormalizingJsonDecodeErrorKind::InputTooLarge.to_string(),
        "input_too_large"
    );
    assert_eq!(NormalizingJsonDecodeErrorKind::EmptyInput.to_string(), "empty_input");
    assert_eq!(NormalizingJsonDecodeErrorKind::InvalidUtf8.to_string(), "invalid_utf8");
    assert_eq!(NormalizingJsonDecodeErrorKind::InvalidJson.to_string(), "invalid_json");
    assert_eq!(NormalizingJsonDecodeErrorKind::Budget.to_string(), "budget");
    assert_eq!(
        NormalizingJsonDecodeErrorKind::UnexpectedTopLevel.to_string(),
        "unexpected_top_level"
    );
    assert_eq!(NormalizingJsonDecodeErrorKind::Deserialize.to_string(), "deserialize");
}

/// Verifies that decode error kind from str.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_error_kind_from_str() {
    assert_eq!(
        NormalizingJsonDecodeErrorKind::from_str("input_too_large").expect("input_too_large must parse"),
        NormalizingJsonDecodeErrorKind::InputTooLarge
    );
    assert_eq!(
        NormalizingJsonDecodeErrorKind::from_str("empty_input").expect("empty_input must parse"),
        NormalizingJsonDecodeErrorKind::EmptyInput
    );
    assert_eq!(
        NormalizingJsonDecodeErrorKind::from_str("INVALID_UTF8")
            .expect("INVALID_UTF8 must parse without case sensitivity"),
        NormalizingJsonDecodeErrorKind::InvalidUtf8
    );
    assert_eq!(
        NormalizingJsonDecodeErrorKind::from_str("INVALID_JSON")
            .expect("INVALID_JSON must parse without case sensitivity"),
        NormalizingJsonDecodeErrorKind::InvalidJson
    );
    assert_eq!(
        NormalizingJsonDecodeErrorKind::from_str("BUDGET").expect("BUDGET must parse without case sensitivity"),
        NormalizingJsonDecodeErrorKind::Budget
    );
    assert_eq!(
        NormalizingJsonDecodeErrorKind::from_str("unexpected_top_level").expect("unexpected_top_level must parse"),
        NormalizingJsonDecodeErrorKind::UnexpectedTopLevel
    );
    assert_eq!(
        NormalizingJsonDecodeErrorKind::from_str("deserialize").expect("deserialize must parse"),
        NormalizingJsonDecodeErrorKind::Deserialize
    );
    assert_eq!(
        NormalizingJsonDecodeErrorKind::from_str("unsupported"),
        Err("unknown NormalizingJsonDecodeErrorKind"),
    );
}
