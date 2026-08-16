// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Public compatibility tests for serde_json's private protocols.

mod budgeted_private_value_tests;
mod json_encode_serializer_tests;
mod json_private_tests;

use qubit_budget::json::JsonEncodeLimits;
use qubit_budget::json::JsonEncodeSession;
use qubit_json::text::JsonEncodeError;
use qubit_json::text::JsonTextEncoder;
use serde::Serialize;
use serde::Serializer;
use serde::ser::SerializeStruct;
use serde_json::Number;
use serde_json::value::RawValue;

/// Simulates serde_json's private raw-value protocol with invalid text.
struct InvalidRawValue;

impl Serialize for InvalidRawValue {
    /// Emits an invalid private raw JSON payload.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct(
            concat!("$", "serde_json", ":", ":private::RawValue"),
            1,
        )?;
        state.serialize_field(
            concat!("$", "serde_json", ":", ":private::RawValue"),
            "[",
        )?;
        state.end()
    }
}

/// Preserves serde_json's private number and raw-value encodings.
#[test]
fn test_encoder_preserves_private_number_and_raw_value_protocol() {
    let number: Number = "123456789012345678901234567890"
        .parse()
        .expect("valid number");
    let raw = RawValue::from_string("{\"ok\":true}".to_owned())
        .expect("valid raw JSON");
    let mut session = JsonEncodeSession::owned(JsonEncodeLimits::empty());
    let bytes = JsonTextEncoder::new(&mut session)
        .to_vec(&(&number, &raw))
        .expect("private serde_json shapes encode");
    assert_eq!(bytes, br#"[123456789012345678901234567890,{"ok":true}]"#,);
}

/// Reports invalid private raw text as a syntax-specific encode error.
#[test]
fn test_encoder_reports_invalid_private_raw_value_as_syntax_error() {
    let mut session = JsonEncodeSession::owned(JsonEncodeLimits::empty());
    let error = JsonTextEncoder::new(&mut session)
        .to_vec(&InvalidRawValue)
        .expect_err("invalid private raw JSON must fail");

    assert!(matches!(error, JsonEncodeError::InvalidRawJson(_)));
}
