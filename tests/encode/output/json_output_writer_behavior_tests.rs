// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests bounded JSON output error precedence.

use qubit_budget::json::JsonResource;
use qubit_json::encode::JsonEncodeErrorKind;
use qubit_json::encode::JsonEncoder;
use serde::Serialize;
use serde::Serializer;
use serde::ser::Error as _;
use serde::ser::SerializeSeq;
use serde::ser::SerializeStruct;

use crate::encode::json_encode_test_support::encode;
use crate::fixtures::JsonTestLimits;

/// Value that masks an inner serializer failure with its own Serde error.
struct MaskedString<'a>(&'a str);

impl Serialize for MaskedString<'_> {
    /// Emits one string and replaces any inner error with a custom error.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer
            .serialize_str(self.0)
            .map_err(|_| S::Error::custom("masked serializer error"))
    }
}

/// Value that ignores a value-budget error before triggering an output error.
struct ValueThenOutputViolation;

impl Serialize for ValueThenOutputViolation {
    /// Preserves traversal after the first failure to exercise error ordering.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut sequence = serializer.serialize_seq(None)?;
        let _ = sequence.serialize_element("x");
        let _ = sequence.serialize_element(&true);
        Err(S::Error::custom("masked serializer errors"))
    }
}

/// Invalid raw JSON whose syntax failure is followed by a custom Serde error.
struct MaskedInvalidRawValue;

impl Serialize for MaskedInvalidRawValue {
    /// Records invalid raw JSON, then attempts to mask that earlier failure.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct(concat!("$", "serde_json", ":", ":private::RawValue"), 1)?;
        state.serialize_field(concat!("$", "serde_json", ":", ":private::RawValue"), "[")?;
        state.end().map_err(|_| S::Error::custom("masked raw JSON error"))
    }
}

/// Verifies output-budget errors take precedence over masked Serde errors.
#[test]
fn test_json_output_buffer_preserves_output_budget_error_precedence() {
    let mut session = JsonTestLimits::new().max_output_bytes(2).encode_session();

    let error = encode(&MaskedString("hello"), &mut session)
        .expect_err("the output writer must retain its original budget error");

    let error = error
        .into_budget_error()
        .expect("expected the original output budget error");
    assert_eq!(
        error
            .budget_error()
            .expect("the error must contain a budget failure")
            .resource(),
        &JsonResource::OutputBytes,
    );
}

/// Verifies value-budget errors take precedence over masked Serde errors.
#[test]
fn test_json_encoder_preserves_value_budget_error_precedence() {
    let mut session = JsonTestLimits::new().max_string_bytes(2).encode_session();

    let error = encode(&MaskedString("hello"), &mut session)
        .expect_err("the serializer must retain its original value-budget error");

    let error = error
        .into_budget_error()
        .expect("expected the original string budget error");
    assert_eq!(
        error
            .budget_error()
            .expect("the error must contain a budget failure")
            .resource(),
        &JsonResource::StringBytes,
    );
}

/// Verifies the first budget violation wins across value and output checks.
#[test]
fn test_json_encoder_preserves_chronological_budget_error_precedence() {
    let mut session = JsonTestLimits::new()
        .max_output_bytes(3)
        .max_string_bytes(0)
        .encode_session();

    let error = encode(&ValueThenOutputViolation, &mut session)
        .expect_err("the encoder must preserve the first ignored budget error");

    let error = error.into_budget_error().expect("expected the first budget error");
    assert_eq!(
        error
            .budget_error()
            .expect("the error must contain a budget failure")
            .resource(),
        &JsonResource::StringBytes,
    );
}

/// Verifies invalid raw JSON wins over a later custom Serde error.
#[test]
fn test_json_output_writer_preserves_raw_json_error_precedence() {
    let mut encoder = JsonEncoder::unlimited();
    let mut output = Vec::new();

    let error = encoder
        .write_incremental(&mut output, &MaskedInvalidRawValue)
        .expect_err("invalid raw JSON must remain the primary error");

    assert_eq!(error.kind(), JsonEncodeErrorKind::InvalidRawJson);
    assert!(error.syntax_error().is_some());
}
