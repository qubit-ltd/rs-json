// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests bounded JSON output error precedence.

use qubit_budget::json::JsonEncodeLimits;
use qubit_budget::json::JsonResource;
use qubit_json::encode::JsonEncodeErrorKind;
use qubit_json::encode::JsonEncoder;
use qubit_json::encode::JsonSerializationErrorKind;
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

const NUMBER_TOKEN: &str = concat!("$", "serde_json", ":", ":private::Number");

/// Ignores a nested non-finite number failure before ending the sequence.
struct SwallowedNonFiniteElement;

impl Serialize for SwallowedNonFiniteElement {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut sequence = serializer.serialize_seq(Some(1))?;
        let _ = sequence.serialize_element(&f64::NAN);
        sequence.end()
    }
}

/// Emits an invalid private Number payload whose failure can be swallowed.
struct NonFinitePrivateNumber;

impl Serialize for NonFinitePrivateNumber {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct(NUMBER_TOKEN, 1)?;
        state.serialize_field(NUMBER_TOKEN, "1e400")?;
        state.end()
    }
}

/// Ignores a nested private Number failure before ending the sequence.
struct SwallowedPrivateNumberElement;

impl Serialize for SwallowedPrivateNumberElement {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut sequence = serializer.serialize_seq(Some(1))?;
        let _ = sequence.serialize_element(&NonFinitePrivateNumber);
        sequence.end()
    }
}

#[derive(Clone, Copy, Debug)]
enum OutputMode {
    Vec,
    Buffered,
    Incremental,
}

type SwallowedNestedOutput = (
    Result<(), qubit_json::encode::JsonEncodeError<JsonResource>>,
    Vec<u8>,
    Option<usize>,
    Option<usize>,
);

fn run_swallowed_nested<T: Serialize>(value: &T, mode: OutputMode, finite: bool) -> SwallowedNestedOutput {
    let limits = if finite {
        JsonEncodeLimits::builder().max_nodes(64).max_output_bytes(256).build()
    } else {
        JsonEncodeLimits::new()
    };
    let mut encoder = JsonEncoder::with_limits(limits);
    let mut output = Vec::new();
    let result = match mode {
        OutputMode::Vec => encoder.to_vec(value).map(|bytes| {
            output = bytes;
        }),
        OutputMode::Buffered => encoder.write_buffered(&mut output, value),
        OutputMode::Incremental => encoder.write_incremental(&mut output, value),
    };
    let used_nodes = encoder.session().value_budget().used_nodes();
    let used_output_bytes = encoder.session().output_budget().map(|budget| budget.used());
    (result, output, used_nodes, used_output_bytes)
}

fn check_swallowed_nested_error(
    result: Result<(), qubit_json::encode::JsonEncodeError<JsonResource>>,
    output: &[u8],
    used_nodes: Option<usize>,
    used_output_bytes: Option<usize>,
    mode: OutputMode,
    finite: bool,
) -> Vec<String> {
    let mut issues = Vec::new();
    let safe_bytes = String::from_utf8_lossy(output).replace("1e400", "<redacted>");
    let context = format!(
        "mode={mode:?}, finite={finite}, bytes={safe_bytes:?}, output_len={}, used_nodes={used_nodes:?}, used_output_bytes={used_output_bytes:?}",
        output.len(),
    );
    match result {
        Ok(()) => issues.push(format!("{context}: operation unexpectedly succeeded")),
        Err(error) => {
            if error.kind() != JsonEncodeErrorKind::Serialize {
                issues.push(format!("{context}: expected Serialize, got {:?}", error.kind()));
            }
            let serialization_kind = error.serialization_error().map(|source| source.kind());
            if serialization_kind != Some(JsonSerializationErrorKind::NonFiniteFloat) {
                issues.push(format!(
                    "{context}: expected NonFiniteFloat, got {serialization_kind:?}"
                ));
            }
            let diagnostic = format!("{error:?} {}", error.serialization_error().expect("Serialize source"));
            if diagnostic.contains("1e400") || diagnostic.contains("swallowed custom text") {
                issues.push(format!("{context}: sensitive serialization detail leaked"));
            }
            if finite && used_nodes != Some(0) {
                issues.push(format!("{context}: failed value transaction was committed"));
            }
            if !finite && used_nodes.is_some() {
                issues.push(format!("{context}: unlimited node budget was expected"));
            }
            match mode {
                OutputMode::Vec | OutputMode::Buffered => {
                    if !output.is_empty() {
                        issues.push(format!("{context}: failed buffered operation published bytes"));
                    }
                    if finite && used_output_bytes != Some(0) {
                        issues.push(format!("{context}: failed buffered operation charged output bytes"));
                    }
                    if !finite && used_output_bytes.is_some() {
                        issues.push(format!("{context}: unlimited output budget was expected"));
                    }
                }
                OutputMode::Incremental => {
                    if finite && used_output_bytes != Some(output.len()) {
                        issues.push(format!("{context}: incremental bytes and output charge differ"));
                    }
                    if !finite && used_output_bytes.is_some() {
                        issues.push(format!("{context}: unlimited output budget was expected"));
                    }
                }
            }
        }
    }
    issues
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

#[test]
fn test_json_output_writer_rejects_swallowed_nested_non_finite_errors() {
    let mut issues = Vec::new();
    for mode in [OutputMode::Vec, OutputMode::Buffered, OutputMode::Incremental] {
        for finite in [false, true] {
            let (result, output, used_nodes, used_output_bytes) =
                run_swallowed_nested(&SwallowedNonFiniteElement, mode, finite);
            issues.extend(check_swallowed_nested_error(
                result,
                &output,
                used_nodes,
                used_output_bytes,
                mode,
                finite,
            ));
        }
    }
    assert!(issues.is_empty(), "{}", issues.join("\n"));
}

#[test]
fn test_json_output_writer_rejects_swallowed_nested_private_number_errors() {
    let mut issues = Vec::new();
    for mode in [OutputMode::Vec, OutputMode::Buffered, OutputMode::Incremental] {
        for finite in [false, true] {
            let (result, output, used_nodes, used_output_bytes) =
                run_swallowed_nested(&SwallowedPrivateNumberElement, mode, finite);
            issues.extend(check_swallowed_nested_error(
                result,
                &output,
                used_nodes,
                used_output_bytes,
                mode,
                finite,
            ));
        }
    }
    assert!(issues.is_empty(), "{}", issues.join("\n"));
}
