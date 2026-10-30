// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Fuzz assertions for serde_json's private RawValue and Number protocols.

use std::cell::Cell;
use std::fmt;

use qubit_budget::ResourceLimit;
use qubit_budget::StructureLimits;
use qubit_budget::json::JsonEncodeLimits;
use qubit_budget::json::JsonEncodeSession;
use qubit_budget::json::JsonResource;
use qubit_budget::json::JsonValueLimits;
use qubit_json::encode::JsonEncodeErrorKind;
use qubit_json::encode::JsonEncoder;
use qubit_json::encode::JsonSerializationErrorKind;
use qubit_json::encode::JsonSerializerStateError;
use qubit_json::value::JsonValueEncoder;
use serde::Serialize;
use serde::Serializer;
use serde::ser::SerializeStruct;

use crate::json_encode_invariants::split_encode_input;
use crate::json_encode_output::FuzzOutput;
use crate::json_encode_output::OutputMode;
use crate::json_number_contract::normalize_reference_value;
use crate::json_number_contract::parse_strict_reference_number;

const RAW_NAME: &str = concat!("$", "serde_json", ":", ":private::RawValue");
const NUMBER_NAME: &str = concat!("$", "serde_json", ":", ":private::Number");
const RAW_FIELD: &str = concat!("$", "serde_json", ":", ":private::RawValue");
const NUMBER_FIELD: &str = concat!("$", "serde_json", ":", ":private::Number");

fn signed_boundary(selector: u8) -> i128 {
    [
        i128::MIN,
        i64::MIN as i128 - 1,
        i64::MIN as i128,
        i64::MAX as i128,
        i64::MAX as i128 + 1,
        u64::MAX as i128,
        u64::MAX as i128 + 1,
        i128::MAX,
    ][usize::from(selector) % 8]
}

fn unsigned_boundary(selector: u8) -> u128 {
    [0, u64::MAX as u128, u64::MAX as u128 + 1, u128::MAX][usize::from(selector) % 4]
}

#[derive(Clone, Copy)]
enum Payload<'a> {
    Text(&'static str, &'a Cell<usize>),
    Integer(i64, &'a Cell<usize>),
    Signed(i128, &'a Cell<usize>),
    Unsigned(u128, &'a Cell<usize>),
    Display(&'static str, usize, Option<usize>, &'a Cell<usize>),
}

struct DisplayText {
    text: &'static str,
    chunks: usize,
    fail_at: Option<usize>,
}

impl fmt::Display for DisplayText {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let chunk_size = self.chunks.max(1);
        let characters: Vec<char> = self.text.chars().collect();
        for (index, chunk) in characters.chunks(chunk_size).enumerate() {
            if self.fail_at == Some(index) {
                return Err(fmt::Error);
            }
            let text: String = chunk.iter().collect();
            formatter.write_str(&text)?;
        }
        Ok(())
    }
}

impl Serialize for Payload<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let callbacks = match self {
            Self::Text(_, callbacks)
            | Self::Integer(_, callbacks)
            | Self::Signed(_, callbacks)
            | Self::Unsigned(_, callbacks)
            | Self::Display(_, _, _, callbacks) => callbacks,
        };
        callbacks.set(callbacks.get() + 1);
        match self {
            Self::Text(value, _) => serializer.serialize_str(value),
            Self::Integer(value, _) => serializer.serialize_i64(*value),
            Self::Signed(value, _) => serializer.serialize_i128(*value),
            Self::Unsigned(value, _) => serializer.serialize_u128(*value),
            Self::Display(text, chunks, fail_at, _) => serializer.collect_str(&DisplayText {
                text,
                chunks: *chunks,
                fail_at: *fail_at,
            }),
        }
    }
}

/// One controlled malformed or valid private-protocol struct.
pub struct ProtocolCase<'a> {
    control: &'a [u8; 9],
    payload: &'static str,
    field_attempts: &'a Cell<usize>,
    payload_callbacks: &'a Cell<usize>,
}

enum IntegerInput {
    Signed(i128),
    Unsigned(u128),
}

impl Serialize for IntegerInput {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::Signed(value) => serializer.serialize_i128(*value),
            Self::Unsigned(value) => serializer.serialize_u128(*value),
        }
    }
}

fn assert_integer_boundary(value: IntegerInput, expected_text: String, in_range: bool, signed: bool) {
    let projected = JsonValueEncoder::new().encode(&value);
    assert_eq!(projected.is_ok(), in_range, "JsonValueEncoder integer range");
    if in_range {
        assert_eq!(
            projected.unwrap(),
            serde_json::from_str::<serde_json::Value>(&expected_text).expect("in-range integer parses")
        );
    } else {
        assert_eq!(
            projected.unwrap_err().kind(),
            JsonSerializationErrorKind::IntegerOutOfRange {
                signedness: if signed {
                    qubit_json::encode::JsonIntegerSignedness::Signed
                } else {
                    qubit_json::encode::JsonIntegerSignedness::Unsigned
                }
            }
        );
    }
    let mut encoder = JsonEncoder::unlimited();
    let encoded = encoder.to_vec(&value);
    assert_eq!(encoded.is_ok(), in_range, "JsonEncoder integer range");
    if in_range {
        assert_eq!(encoded.unwrap(), expected_text.as_bytes());
    } else {
        assert_eq!(encoded.unwrap_err().kind(), JsonEncodeErrorKind::Serialize);
    }
}

fn assert_integer_boundary_matrix(selector: u8) {
    let signed = [
        i128::MIN,
        i64::MIN as i128 - 1,
        i64::MIN as i128,
        -1,
        0,
        i64::MAX as i128,
        i64::MAX as i128 + 1,
        u64::MAX as i128,
        u64::MAX as i128 + 1,
        i128::MAX,
    ];
    let unsigned = [0, 1, u64::MAX as u128, u64::MAX as u128 + 1, u128::MAX];
    let signed_value = signed[usize::from(selector) % signed.len()];
    assert_integer_boundary(
        IntegerInput::Signed(signed_value),
        signed_value.to_string(),
        i64::try_from(signed_value).is_ok() || (signed_value >= 0 && u64::try_from(signed_value).is_ok()),
        true,
    );
    let unsigned_value = unsigned[usize::from(selector) % unsigned.len()];
    assert_integer_boundary(
        IntegerInput::Unsigned(unsigned_value),
        unsigned_value.to_string(),
        u64::try_from(unsigned_value).is_ok(),
        false,
    );
}

/// Checks the public signed and unsigned 128-bit boundary contract.
pub fn assert_json_integer_boundary_invariants(selector: u8) {
    assert_integer_boundary_matrix(selector);
}

impl<'a> ProtocolCase<'a> {
    /// Creates a protocol case from a bounded fuzz control header.
    #[must_use]
    pub const fn new(
        control: &'a [u8; 9],
        payload: &'static str,
        field_attempts: &'a Cell<usize>,
        payload_callbacks: &'a Cell<usize>,
    ) -> Self {
        Self {
            control,
            payload,
            field_attempts,
            payload_callbacks,
        }
    }
}

impl Serialize for ProtocolCase<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let number = self.control[0] & 1 != 0;
        let name = if number { NUMBER_NAME } else { RAW_NAME };
        let field = match self.control[1] % 4 {
            0 => {
                if number {
                    NUMBER_FIELD
                } else {
                    RAW_FIELD
                }
            }
            1 => "wrong-field",
            2 => {
                if number {
                    RAW_FIELD
                } else {
                    NUMBER_FIELD
                }
            }
            _ => {
                if number {
                    NUMBER_FIELD
                } else {
                    RAW_FIELD
                }
            }
        };
        let declared = match self.control[2] % 3 {
            0 => 0,
            1 => 1,
            _ => 2,
        };
        let operations = usize::from(self.control[3] % 9);
        let mut compound = serializer.serialize_struct(name, declared)?;
        for index in 0..operations {
            self.field_attempts.set(self.field_attempts.get() + 1);
            let key = if self.control[4] & 1 != 0 && index > 0 {
                "wrong-field"
            } else {
                field
            };
            let value = match self.control[5] % 4 {
                0 if self.control[7] & 1 != 0 => {
                    let raw = self.control[8];
                    let fail_selector = usize::from(raw / 8) % 9;
                    Payload::Display(
                        self.payload,
                        1 + usize::from(raw % 8),
                        if fail_selector > 0 {
                            Some(fail_selector - 1)
                        } else {
                            None
                        },
                        self.payload_callbacks,
                    )
                }
                0 => Payload::Text(self.payload, self.payload_callbacks),
                1 => Payload::Integer(7, self.payload_callbacks),
                2 => Payload::Signed(signed_boundary(self.control[6]), self.payload_callbacks),
                _ => Payload::Unsigned(unsigned_boundary(self.control[6]), self.payload_callbacks),
            };
            let _ = compound.serialize_field(key, &value);
        }
        compound.end()
    }
}

fn selected_payload(control: &[u8; 9]) -> &'static str {
    const NUMBERS: &[&str] = &["0", "-0", "1.25", "1e2", "1e400", "-1e400", "01"];
    const RAW: &[&str] = &["null", r#"{"ok":true}"#, "[", "[[[[0]]]]"];
    if control[0] & 1 == 0 {
        RAW[usize::from(control[6]) % RAW.len()]
    } else {
        NUMBERS[usize::from(control[6]) % NUMBERS.len()]
    }
}

fn reference_payload(
    control: &[u8; 9],
    payload: &'static str,
) -> Result<serde_json::Value, JsonSerializationErrorKind> {
    if control[0] & 1 != 0 {
        return parse_strict_reference_number(payload).map(serde_json::Value::Number);
    }
    let value =
        serde_json::from_str::<serde_json::Value>(payload).map_err(|_| JsonSerializationErrorKind::InvalidRawValue)?;
    normalize_reference_value(&value).map_err(|_| JsonSerializationErrorKind::InvalidRawValue)
}

fn independent_oracle(control: &[u8; 9], payload: &'static str) -> Option<serde_json::Value> {
    if !matches!(control[1] % 4, 0 | 3) || control[2] % 3 != 1 || control[3] % 9 != 1 || !control[5].is_multiple_of(4) {
        return None;
    }
    if control[7] & 1 != 0 {
        let raw = control[8];
        let chunks = 1 + usize::from(raw % 8);
        let fail_selector = usize::from(raw / 8) % 9;
        if fail_selector > 0 {
            let fail_index = fail_selector - 1;
            let chunk_count = payload.chars().count().div_ceil(chunks);
            if fail_index < chunk_count {
                return None;
            }
        }
    }
    reference_payload(control, payload).ok()
}

fn expected_value_error(control: &[u8; 9], payload: &'static str) -> Option<JsonSerializationErrorKind> {
    let invalid_state = || JsonSerializationErrorKind::InvalidSerializerState {
        reason: if control[0] & 1 == 0 {
            JsonSerializerStateError::InvalidRawValueProtocol
        } else {
            JsonSerializerStateError::InvalidNumberProtocol
        },
    };
    if control[2] % 3 != 1 || control[3].is_multiple_of(9) {
        return Some(invalid_state());
    }
    if !matches!(control[1] % 4, 0 | 3) || !control[5].is_multiple_of(4) {
        return Some(invalid_state());
    }
    if control[7] & 1 != 0 {
        let raw = control[8];
        let chunks = 1 + usize::from(raw % 8);
        let fail_selector = usize::from(raw / 8) % 9;
        if fail_selector > 0 && fail_selector - 1 < payload.chars().count().div_ceil(chunks) {
            return Some(JsonSerializationErrorKind::DisplayFormattingFailed);
        }
    }
    // The strict value compound detects a second field before finalizing
    // either RawValue or Number text.
    if control[3] % 9 > 1 {
        return Some(invalid_state());
    }
    if let Err(error) = reference_payload(control, payload) {
        return Some(error);
    }
    if control[3] % 9 > 1 {
        return Some(invalid_state());
    }
    None
}

fn expected_encode_error(
    control: &[u8; 9],
    value_error: Option<JsonSerializationErrorKind>,
    payload: &'static str,
) -> JsonEncodeErrorKind {
    let raw_payload_reached = control[0] & 1 == 0
        && control[2] % 3 == 1
        && !control[3].is_multiple_of(9)
        && matches!(control[1] % 4, 0 | 3)
        && control[5].is_multiple_of(4)
        && (control[7] & 1 == 0 || {
            let raw = control[8];
            let chunks = 1 + usize::from(raw % 8);
            let fail_selector = usize::from(raw / 8) % 9;
            fail_selector == 0 || fail_selector > payload.chars().count().div_ceil(chunks)
        });
    if value_error == Some(JsonSerializationErrorKind::InvalidRawValue)
        || (raw_payload_reached && reference_payload(control, payload).is_err())
    {
        JsonEncodeErrorKind::InvalidRawJson
    } else {
        JsonEncodeErrorKind::Serialize
    }
}

fn encode_limits(output: usize, nodes: usize, payload: usize) -> JsonEncodeLimits<JsonResource, usize> {
    let structure = StructureLimits::builder().nodes_limit(ResourceLimit::new(JsonResource::Nodes, nodes));
    let values = JsonValueLimits::<JsonResource, usize>::builder()
        .structure_limits(structure)
        .payload_bytes_limit(ResourceLimit::new(JsonResource::PayloadBytes, payload))
        .build();
    JsonEncodeLimits::<JsonResource, usize>::builder()
        .output_bytes_limit(ResourceLimit::new(JsonResource::OutputBytes, output))
        .value_limits(values)
        .build()
}

fn unbounded_encoder() -> JsonEncoder<'static, JsonResource, usize> {
    JsonEncoder::new(JsonEncodeSession::from_limits(encode_limits(
        usize::MAX,
        usize::MAX,
        usize::MAX,
    )))
}

fn case<'a>(
    control: &'a [u8; 9],
    payload: &'static str,
    field_attempts: &'a Cell<usize>,
    payload_callbacks: &'a Cell<usize>,
) -> ProtocolCase<'a> {
    ProtocolCase::new(control, payload, field_attempts, payload_callbacks)
}

fn assert_protocol_budget_edges(value: &ProtocolCase<'_>, expected: &[u8], selectors: [u8; 3], baseline: [usize; 3]) {
    for resource in 0..3 {
        let mut limits = baseline;
        limits[resource] =
            crate::json_encode_invariants::select_encode_limit(u16::from(selectors[resource]), baseline[resource]);
        let fits = limits[resource] >= baseline[resource];

        let mut vector_encoder = JsonEncoder::new(JsonEncodeSession::from_limits(encode_limits(
            limits[0], limits[1], limits[2],
        )));
        let vector = vector_encoder.to_vec(value);
        assert_eq!(vector.is_ok(), fits, "to_vec resource selector {resource}");
        if fits {
            assert_eq!(vector.unwrap(), expected);
        } else {
            assert_eq!(vector.unwrap_err().kind(), JsonEncodeErrorKind::Budget);
            let session = vector_encoder.into_session();
            assert_eq!(session.output_budget().expect("output tracked").used(), 0);
            assert_eq!(session.value_budget().used_nodes(), Some(0));
            assert_eq!(session.value_budget().used_payload_bytes(), Some(0));
        }

        let mut buffered_encoder = JsonEncoder::new(JsonEncodeSession::from_limits(encode_limits(
            limits[0], limits[1], limits[2],
        )));
        let mut buffered_bytes = Vec::new();
        let buffered = buffered_encoder.write_buffered(&mut buffered_bytes, value);
        assert_eq!(buffered.is_ok(), fits, "write_buffered resource selector {resource}");
        if fits {
            assert_eq!(buffered_bytes, expected);
        } else {
            assert_eq!(buffered.unwrap_err().kind(), JsonEncodeErrorKind::Budget);
            assert!(buffered_bytes.is_empty(), "buffered failure writes no output");
            let session = buffered_encoder.into_session();
            assert_eq!(session.value_budget().used_nodes(), Some(0));
            assert_eq!(session.value_budget().used_payload_bytes(), Some(0));
        }

        let mut output = Vec::new();
        let mut incremental_encoder = JsonEncoder::new(JsonEncodeSession::from_limits(encode_limits(
            limits[0], limits[1], limits[2],
        )));
        let incremental = incremental_encoder.write_incremental(&mut output, value);
        assert_eq!(
            incremental.is_ok(),
            fits,
            "write_incremental resource selector {resource}"
        );
        let session = incremental_encoder.into_session();
        let used_output = session.output_budget().expect("output tracking enabled").used();
        assert_eq!(used_output, output.len());
        if fits {
            assert_eq!(output, expected);
        } else {
            assert_eq!(incremental.unwrap_err().kind(), JsonEncodeErrorKind::Budget);
            assert_eq!(session.value_budget().used_nodes(), Some(0));
            assert_eq!(session.value_budget().used_payload_bytes(), Some(0));
        }
    }
}

/// Checks private protocol admission, output, accounting, and rollback
/// invariants.
pub fn assert_json_serde_protocol_invariants(data: &[u8]) {
    let Some((control, selector_bytes)) = split_encode_input(data) else {
        return;
    };
    let payload = selected_payload(control);
    assert_integer_boundary_matrix(control[6]);
    let oracle = independent_oracle(control, payload);
    let expected_error = expected_value_error(control, payload);
    let field_attempts = Cell::new(0);
    let payload_callbacks = Cell::new(0);
    let value = case(control, payload, &field_attempts, &payload_callbacks);

    let projected = JsonValueEncoder::new().encode(&value);
    assert!(field_attempts.get() <= 8);
    assert!(payload_callbacks.get() <= field_attempts.get());
    assert_eq!(
        projected.is_ok(),
        oracle.is_some(),
        "strict value result must match independent JSON oracle"
    );
    match (projected, oracle.as_ref()) {
        (Ok(projected), Some(oracle)) => assert_eq!(projected, *oracle),
        (Err(error), None) => assert_eq!(
            error.kind(),
            expected_error.expect("oracle rejection has classified error"),
            "JsonValueEncoder error classification for control={control:?}, payload={payload:?}, payload_kind={}",
            control[5] % 4
        ),
        _ => unreachable!("projected result and independent oracle must agree"),
    }

    // Private RawValue and Number protocols carry their original JSON lexeme;
    // serializing the parsed Value would normalize tokens such as -0.
    let expected_bytes = oracle.as_ref().map(|_| payload.as_bytes().to_vec());
    let mut vector_encoder = unbounded_encoder();
    let vector = vector_encoder.to_vec(&value);
    assert_eq!(
        vector.is_ok(),
        oracle.is_some(),
        "to_vec result must match independent JSON oracle"
    );
    if let Some(expected) = expected_bytes.as_ref() {
        assert_eq!(vector.expect("oracle success must encode"), *expected);
    } else {
        assert_eq!(
            vector.unwrap_err().kind(),
            expected_encode_error(control, expected_error, payload),
            "to_vec failure classification for control={control:?}, value_error={expected_error:?}"
        );
    }

    let mut buffered = Vec::new();
    let mut buffered_encoder = unbounded_encoder();
    let buffered_result = buffered_encoder.write_buffered(&mut buffered, &value);
    assert_eq!(
        buffered_result.is_ok(),
        oracle.is_some(),
        "buffered result must match independent JSON oracle"
    );
    if let Some(expected) = expected_bytes.as_ref() {
        assert_eq!(buffered, *expected);
    } else {
        assert!(buffered.is_empty());
        assert_eq!(
            buffered_result.unwrap_err().kind(),
            expected_encode_error(control, expected_error, payload),
            "buffered failure classification for control={control:?}, value_error={expected_error:?}"
        );
    }

    let mode = match control[7] % 3 {
        0 => OutputMode::Complete,
        1 => OutputMode::InterruptOnce,
        _ => OutputMode::BrokenPipe,
    };
    let max_output = usize::from(control[8]);
    let writer_remaining = if matches!(mode, OutputMode::BrokenPipe) {
        max_output
    } else {
        usize::MAX
    };
    let mut buffered_writer = FuzzOutput::new(mode, writer_remaining, 1 + usize::from(control[6] % 16));
    let mut buffered_writer_encoder = unbounded_encoder();
    let buffered_writer_result = buffered_writer_encoder.write_buffered(&mut buffered_writer, &value);
    if let Some(expected) = expected_bytes.as_ref() {
        if matches!(mode, OutputMode::BrokenPipe) && max_output < expected.len() {
            assert_eq!(buffered_writer_result.unwrap_err().kind(), JsonEncodeErrorKind::Write);
            assert_eq!(buffered_writer.bytes(), &expected[..buffered_writer.bytes().len()]);
            let session = buffered_writer_encoder.into_session();
            assert_eq!(
                session.output_budget().expect("output budget tracked").used(),
                buffered_writer.bytes().len(),
                "accepted buffered-writer bytes remain charged after failure"
            );
            assert_eq!(session.value_budget().used_nodes(), Some(0));
            assert_eq!(session.value_budget().used_payload_bytes(), Some(0));
        } else {
            assert!(buffered_writer_result.is_ok());
            assert_eq!(buffered_writer.bytes(), expected);
        }
    } else {
        assert!(buffered_writer.bytes().is_empty());
        assert_eq!(
            buffered_writer_result.unwrap_err().kind(),
            expected_encode_error(control, expected_error, payload),
            "buffered writer failure classification for control={control:?}, value_error={expected_error:?}"
        );
    }

    let remaining = if matches!(mode, OutputMode::BrokenPipe) {
        max_output
    } else {
        usize::MAX
    };
    let mut incremental_output = FuzzOutput::new(mode, remaining, 1 + usize::from(control[6] % 16));
    let mut incremental_encoder = unbounded_encoder();
    let incremental = incremental_encoder.write_incremental(&mut incremental_output, &value);
    let session = incremental_encoder.into_session();
    assert_eq!(
        session.output_budget().expect("output budget tracked").used(),
        incremental_output.bytes().len()
    );
    if let Some(expected) = expected_bytes.as_ref() {
        if matches!(mode, OutputMode::BrokenPipe) && max_output < expected.len() {
            assert_eq!(incremental.unwrap_err().kind(), JsonEncodeErrorKind::Write);
            assert_eq!(
                incremental_output.bytes(),
                &expected[..incremental_output.bytes().len()]
            );
            assert_eq!(session.value_budget().used_nodes(), Some(0));
            assert_eq!(session.value_budget().used_payload_bytes(), Some(0));
        } else {
            assert!(
                incremental.is_ok(),
                "complete/interrupted writer should finish oracle success"
            );
            assert_eq!(incremental_output.bytes(), expected);
        }
    } else {
        let error_kind = incremental.unwrap_err().kind();
        if matches!(mode, OutputMode::BrokenPipe) {
            assert!(
                error_kind == JsonEncodeErrorKind::Write
                    || error_kind == expected_encode_error(control, expected_error, payload),
                "incremental BrokenPipe failure classification for control={control:?}, value_error={expected_error:?}"
            );
            assert!(incremental_output.bytes().len() <= max_output);
        } else {
            assert_eq!(
                error_kind,
                expected_encode_error(control, expected_error, payload),
                "incremental failure classification for control={control:?}, value_error={expected_error:?}"
            );
        }
        assert_eq!(session.value_budget().used_nodes(), Some(0));
        assert_eq!(session.value_budget().used_payload_bytes(), Some(0));
    }

    if let Some(expected) = expected_bytes.as_ref() {
        let session = {
            let mut baseline_encoder = unbounded_encoder();
            let baseline = baseline_encoder.to_vec(&value).expect("oracle case");
            assert_eq!(baseline, *expected);
            baseline_encoder.into_session()
        };
        let baseline = [
            session.output_budget().expect("output budget").used(),
            session.value_budget().used_nodes().expect("node budget"),
            session.value_budget().used_payload_bytes().expect("payload budget"),
        ];
        let selectors = [
            selector_bytes.first().copied().unwrap_or(control[4]),
            selector_bytes.get(1).copied().unwrap_or(control[5]),
            selector_bytes.get(2).copied().unwrap_or(control[6]),
        ];
        assert_protocol_budget_edges(&value, expected, selectors, baseline);
    }
}
