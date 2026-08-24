// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Public compatibility tests for serde_json's RawValue protocol.

mod budgeted_private_value_tests;
mod json_encode_serializer_tests;
mod json_private_tests;

use qubit_budget::json::JsonEncodeLimits;
use qubit_budget::json::JsonEncodeSession;
use qubit_budget::json::JsonResource;
use qubit_json::encode::JsonEncodeError;
use qubit_json::encode::JsonEncoder;
use serde::Serialize;
use serde::Serializer;
use serde::ser::SerializeStruct;
use serde_json::value::RawValue;

/// Simulates serde_json's private raw-value protocol with invalid text.
struct InvalidRawValue;

impl Serialize for InvalidRawValue {
    /// Emits an invalid private raw JSON payload.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct(concat!("$", "serde_json", ":", ":private::RawValue"), 1)?;
        state.serialize_field(concat!("$", "serde_json", ":", ":private::RawValue"), "[")?;
        state.end()
    }
}

/// Preserves serde_json's private raw-value encoding.
#[test]
fn test_encoder_preserves_private_raw_value_protocol() {
    let raw = RawValue::from_string("{\"ok\":true}".to_owned()).expect("valid raw JSON");
    let session = JsonEncodeSession::owned(JsonEncodeLimits::<JsonResource, usize>::builder().build());
    let bytes = JsonEncoder::new(session)
        .to_vec(&raw)
        .expect("serde_json RawValue should encode");
    assert_eq!(bytes, br#"{"ok":true}"#);
}

/// Reports invalid private raw text as a syntax-specific encode error.
#[test]
fn test_encoder_reports_invalid_private_raw_value_as_syntax_error() {
    let session = JsonEncodeSession::owned(JsonEncodeLimits::<JsonResource, usize>::builder().build());
    let error = JsonEncoder::new(session)
        .to_vec(&InvalidRawValue)
        .expect_err("invalid private raw JSON must fail");

    assert!(matches!(error, JsonEncodeError::InvalidRawJson(_)));
}
