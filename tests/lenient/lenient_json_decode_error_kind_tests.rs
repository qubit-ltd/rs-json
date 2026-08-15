// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests for the public `LenientJsonDecodeErrorKind` type.

use std::str::FromStr;

use qubit_json::lenient::LenientJsonDecodeErrorKind;

/// Verifies that decode error kind display uses snake case names.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_error_kind_display_uses_snake_case_names() {
    assert_eq!(
        LenientJsonDecodeErrorKind::InputTooLarge.to_string(),
        "input_too_large"
    );
    assert_eq!(
        LenientJsonDecodeErrorKind::EmptyInput.to_string(),
        "empty_input"
    );
    assert_eq!(
        LenientJsonDecodeErrorKind::InvalidUtf8.to_string(),
        "invalid_utf8"
    );
    assert_eq!(
        LenientJsonDecodeErrorKind::InvalidJson.to_string(),
        "invalid_json"
    );
    assert_eq!(LenientJsonDecodeErrorKind::Budget.to_string(), "budget");
    assert_eq!(
        LenientJsonDecodeErrorKind::UnexpectedTopLevel.to_string(),
        "unexpected_top_level"
    );
    assert_eq!(
        LenientJsonDecodeErrorKind::Deserialize.to_string(),
        "deserialize"
    );
}

/// Verifies that decode error kind from str.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_error_kind_from_str() {
    assert_eq!(
        LenientJsonDecodeErrorKind::from_str("input_too_large")
            .expect("input_too_large must parse"),
        LenientJsonDecodeErrorKind::InputTooLarge
    );
    assert_eq!(
        LenientJsonDecodeErrorKind::from_str("empty_input").expect("empty_input must parse"),
        LenientJsonDecodeErrorKind::EmptyInput
    );
    assert_eq!(
        LenientJsonDecodeErrorKind::from_str("INVALID_UTF8")
            .expect("INVALID_UTF8 must parse without case sensitivity"),
        LenientJsonDecodeErrorKind::InvalidUtf8
    );
    assert_eq!(
        LenientJsonDecodeErrorKind::from_str("INVALID_JSON")
            .expect("INVALID_JSON must parse without case sensitivity"),
        LenientJsonDecodeErrorKind::InvalidJson
    );
    assert_eq!(
        LenientJsonDecodeErrorKind::from_str("BUDGET")
            .expect("BUDGET must parse without case sensitivity"),
        LenientJsonDecodeErrorKind::Budget
    );
    assert_eq!(
        LenientJsonDecodeErrorKind::from_str("unexpected_top_level")
            .expect("unexpected_top_level must parse"),
        LenientJsonDecodeErrorKind::UnexpectedTopLevel
    );
    assert_eq!(
        LenientJsonDecodeErrorKind::from_str("deserialize").expect("deserialize must parse"),
        LenientJsonDecodeErrorKind::Deserialize
    );
    assert_eq!(
        LenientJsonDecodeErrorKind::from_str("unsupported"),
        Err("unknown LenientJsonDecodeErrorKind"),
    );
}
