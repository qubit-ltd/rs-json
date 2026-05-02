/*******************************************************************************
 *
 *    Copyright (c) 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
//! Tests for the public `JsonDecodeErrorKind` type in `json_decode_error_kind.rs`.
//!

use qubit_json::JsonDecodeErrorKind;
use std::str::FromStr;

#[test]
fn test_decode_error_kind_display_uses_snake_case_names() {
    assert_eq!(
        JsonDecodeErrorKind::InputTooLarge.to_string(),
        "input_too_large"
    );
    assert_eq!(JsonDecodeErrorKind::EmptyInput.to_string(), "empty_input");
    assert_eq!(JsonDecodeErrorKind::InvalidJson.to_string(), "invalid_json");
    assert_eq!(
        JsonDecodeErrorKind::UnexpectedTopLevel.to_string(),
        "unexpected_top_level"
    );
    assert_eq!(JsonDecodeErrorKind::Deserialize.to_string(), "deserialize");
}

#[test]
fn test_decode_error_kind_from_str() {
    assert_eq!(
        JsonDecodeErrorKind::from_str("input_too_large").unwrap(),
        JsonDecodeErrorKind::InputTooLarge
    );
    assert_eq!(
        JsonDecodeErrorKind::from_str("empty_input").unwrap(),
        JsonDecodeErrorKind::EmptyInput
    );
    assert_eq!(
        JsonDecodeErrorKind::from_str("INVALID_JSON").unwrap(),
        JsonDecodeErrorKind::InvalidJson
    );
    assert_eq!(
        JsonDecodeErrorKind::from_str("unexpected_top_level").unwrap(),
        JsonDecodeErrorKind::UnexpectedTopLevel
    );
    assert_eq!(
        JsonDecodeErrorKind::from_str("deserialize").unwrap(),
        JsonDecodeErrorKind::Deserialize
    );
    assert!(JsonDecodeErrorKind::from_str("unsupported").is_err());
}
