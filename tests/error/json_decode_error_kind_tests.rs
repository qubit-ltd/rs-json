// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests for the public `JsonDecodeErrorKind` type in
//! `json_decode_error_kind.rs`.

use std::str::FromStr;

use qubit_json::lenient::JsonDecodeErrorKind;

/// Verifies that decode error kind display uses snake case names.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_error_kind_display_uses_snake_case_names() {
    assert_eq!(
        JsonDecodeErrorKind::InputTooLarge.to_string(),
        "input_too_large"
    );
    assert_eq!(JsonDecodeErrorKind::EmptyInput.to_string(), "empty_input");
    assert_eq!(JsonDecodeErrorKind::InvalidUtf8.to_string(), "invalid_utf8");
    assert_eq!(JsonDecodeErrorKind::InvalidJson.to_string(), "invalid_json");
    assert_eq!(
        JsonDecodeErrorKind::UnexpectedTopLevel.to_string(),
        "unexpected_top_level"
    );
    assert_eq!(JsonDecodeErrorKind::Deserialize.to_string(), "deserialize");
}

/// Verifies that decode error kind from str.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_error_kind_from_str() {
    assert_eq!(
        JsonDecodeErrorKind::from_str("input_too_large")
            .expect("input_too_large must parse"),
        JsonDecodeErrorKind::InputTooLarge
    );
    assert_eq!(
        JsonDecodeErrorKind::from_str("empty_input")
            .expect("empty_input must parse"),
        JsonDecodeErrorKind::EmptyInput
    );
    assert_eq!(
        JsonDecodeErrorKind::from_str("INVALID_UTF8")
            .expect("INVALID_UTF8 must parse without case sensitivity"),
        JsonDecodeErrorKind::InvalidUtf8
    );
    assert_eq!(
        JsonDecodeErrorKind::from_str("INVALID_JSON")
            .expect("INVALID_JSON must parse without case sensitivity"),
        JsonDecodeErrorKind::InvalidJson
    );
    assert_eq!(
        JsonDecodeErrorKind::from_str("unexpected_top_level")
            .expect("unexpected_top_level must parse"),
        JsonDecodeErrorKind::UnexpectedTopLevel
    );
    assert_eq!(
        JsonDecodeErrorKind::from_str("deserialize")
            .expect("deserialize must parse"),
        JsonDecodeErrorKind::Deserialize
    );
    assert_eq!(
        JsonDecodeErrorKind::from_str("unsupported"),
        Err("unknown JsonDecodeErrorKind"),
    );
}
