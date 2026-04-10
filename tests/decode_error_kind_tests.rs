/*******************************************************************************
 *
 *    Copyright (c) 2026.
 *    Haixing Hu, Qubit Co. Ltd.
 *
 *    All rights reserved.
 *
 ******************************************************************************/
//! Tests for the public `JsonDecodeErrorKind` type in `decode_error_kind.rs`.
//!
//! Author: Haixing Hu

use qubit_json::JsonDecodeErrorKind;

#[test]
fn test_decode_error_kind_display_uses_snake_case_names() {
    assert_eq!(JsonDecodeErrorKind::EmptyInput.to_string(), "empty_input");
    assert_eq!(JsonDecodeErrorKind::InvalidJson.to_string(), "invalid_json");
    assert_eq!(
        JsonDecodeErrorKind::UnexpectedTopLevel.to_string(),
        "unexpected_top_level"
    );
    assert_eq!(JsonDecodeErrorKind::Deserialize.to_string(), "deserialize");
}
