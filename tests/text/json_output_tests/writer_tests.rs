// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests bounded JSON output error precedence.

use qubit_budget::json::JsonResource;
use qubit_json::text::JsonEncodeError;
use serde::Serialize;
use serde::Serializer;
use serde::ser::Error as _;
use serde::ser::SerializeSeq;

use crate::fixtures::JsonTestLimits;
use crate::text::json_encode_test_support::encode;

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

/// Verifies output-budget errors take precedence over masked Serde errors.
#[test]
fn test_json_output_buffer_preserves_output_budget_error_precedence() {
    let mut session = JsonTestLimits::new()
        .with_max_output_bytes(2)
        .encode_session();

    let error = encode(&MaskedString("hello"), &mut session)
        .expect_err("the output writer must retain its original budget error");

    let JsonEncodeError::Budget(error) = error else {
        panic!("expected the original output budget error");
    };
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
    let mut session = JsonTestLimits::new()
        .with_max_string_bytes(2)
        .encode_session();

    let error = encode(&MaskedString("hello"), &mut session)
        .expect_err("the serializer must retain its original value-budget error");

    let JsonEncodeError::Budget(error) = error else {
        panic!("expected the original string budget error");
    };
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
        .with_max_output_bytes(3)
        .with_max_string_bytes(0)
        .encode_session();

    let error = encode(&ValueThenOutputViolation, &mut session)
        .expect_err("the encoder must preserve the first ignored budget error");

    let JsonEncodeError::Budget(error) = error else {
        panic!("expected the first budget error");
    };
    assert_eq!(
        error
            .budget_error()
            .expect("the error must contain a budget failure")
            .resource(),
        &JsonResource::StringBytes,
    );
}
