// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Exercises the exact RawValue and Number private serializer protocols.

#[path = "support/mod.rs"]
mod support;

use qubit_budget::ResourceLimit;
use qubit_budget::StructureLimits;
use qubit_budget::json::JsonEncodeLimits;
use qubit_budget::json::JsonEncodeSession;
use qubit_budget::json::JsonResource;
use qubit_budget::json::JsonValueLimits;
use qubit_json::encode::JsonEncoder;
use qubit_json::encode::JsonSerializerStateError;
use serde::Serialize;
use serde::Serializer;
use serde::ser::SerializeStruct;
use std::fmt::Display;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

const RAW: &str = concat!("$", "serde_json", ":", ":private::RawValue");
const NUMBER: &str = concat!("$", "serde_json", ":", ":private::Number");

#[derive(Clone, Copy)]
enum PrivateKind {
    Raw,
    Number,
}

impl PrivateKind {
    fn token(self) -> &'static str {
        match self {
            Self::Raw => RAW,
            Self::Number => NUMBER,
        }
    }

    fn payload(self) -> &'static str {
        match self {
            Self::Raw => "null",
            Self::Number => "1.25",
        }
    }

    fn error(self) -> JsonSerializerStateError {
        match self {
            Self::Raw => JsonSerializerStateError::InvalidRawValueProtocol,
            Self::Number => JsonSerializerStateError::InvalidNumberProtocol,
        }
    }
}

struct PrivateProtocol {
    kind: PrivateKind,
    declared_len: usize,
    field_count: usize,
    wrong_field: bool,
    payload: &'static str,
}

impl Serialize for PrivateProtocol {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let token = self.kind.token();
        let mut state = serializer.serialize_struct(token, self.declared_len)?;
        let field = if self.wrong_field { "wrong" } else { token };
        for _ in 0..self.field_count {
            state.serialize_field(field, self.payload)?;
        }
        state.end()
    }
}

#[derive(Clone, Copy)]
enum OutputMode {
    Vec,
    Buffered,
    Incremental,
}

fn encoder(finite: bool) -> JsonEncoder<'static, JsonResource, usize> {
    let limits = if finite {
        JsonEncodeLimits::builder()
            .output_bytes_limit(ResourceLimit::new(JsonResource::OutputBytes, 256))
            .value_limits(
                JsonValueLimits::builder()
                    .structure_limits(
                        StructureLimits::builder()
                            .nodes_limit(ResourceLimit::new(JsonResource::Nodes, 64)),
                    )
                    .build(),
            )
            .build()
    } else {
        JsonEncodeLimits::new()
    };
    JsonEncoder::new(JsonEncodeSession::from_limits(limits))
}

fn encode(
    encoder: &mut JsonEncoder<'static, JsonResource, usize>,
    mode: OutputMode,
    value: &PrivateProtocol,
) -> (
    Result<Vec<u8>, qubit_json::encode::JsonEncodeError<JsonResource>>,
    Vec<u8>,
) {
    match mode {
        OutputMode::Vec => (encoder.to_vec(value), Vec::new()),
        OutputMode::Buffered => {
            let mut output = Vec::new();
            let result = encoder
                .write_buffered(&mut output, value)
                .map(|()| output.clone());
            (result, output)
        }
        OutputMode::Incremental => {
            let mut output = Vec::new();
            let result = encoder
                .write_incremental(&mut output, value)
                .map(|()| output.clone());
            (result, output)
        }
    }
}

#[test]
fn test_private_protocol_shape_matrix() {
    let modes = [
        OutputMode::Vec,
        OutputMode::Buffered,
        OutputMode::Incremental,
    ];
    for kind in [PrivateKind::Raw, PrivateKind::Number] {
        for fields in 0..=2 {
            for declared in 0..=2 {
                for finite in [false, true] {
                    for mode in modes {
                        let value = PrivateProtocol {
                            kind,
                            declared_len: declared,
                            field_count: fields,
                            wrong_field: false,
                            payload: kind.payload(),
                        };
                        let mut encoder = encoder(finite);
                        let (result, output) = encode(&mut encoder, mode, &value);
                        let valid = declared == 1 && fields == 1;
                        if valid {
                            assert_eq!(
                                result.expect("one well-formed payload must encode"),
                                value.payload.as_bytes()
                            );
                            assert_eq!(
                                encoder.session().value_budget().used_nodes(),
                                finite.then_some(1)
                            );
                            assert_eq!(
                                encoder
                                    .session()
                                    .output_budget()
                                    .map(|budget| budget.used()),
                                finite.then_some(value.payload.len())
                            );
                        } else {
                            let error = result.expect_err("malformed private protocol must fail");
                            support::assert_private_state_error(&error, kind.error());
                            assert_eq!(
                                encoder.session().value_budget().used_nodes(),
                                finite.then_some(0)
                            );
                            let expected_output = matches!(mode, OutputMode::Incremental)
                                && fields == 2
                                && declared == 1;
                            assert_eq!(
                                encoder
                                    .session()
                                    .output_budget()
                                    .map(|budget| budget.used()),
                                finite.then_some(if expected_output {
                                    value.payload.len()
                                } else {
                                    0
                                }),
                            );
                            match mode {
                                OutputMode::Vec | OutputMode::Buffered => {
                                    assert!(output.is_empty())
                                }
                                OutputMode::Incremental if fields == 2 && declared == 1 => {
                                    assert_eq!(output, value.payload.as_bytes());
                                }
                                OutputMode::Incremental => assert!(output.is_empty()),
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn test_private_protocol_wrong_field_and_invalid_payloads_are_classified() {
    for kind in [PrivateKind::Raw, PrivateKind::Number] {
        let mut encoder = JsonEncoder::unlimited();
        let wrong_field = PrivateProtocol {
            kind,
            declared_len: 1,
            field_count: 1,
            wrong_field: true,
            payload: kind.payload(),
        };
        let error = encoder
            .to_vec(&wrong_field)
            .expect_err("wrong field must fail");
        support::assert_private_state_error(&error, kind.error());
    }

    let malformed_number = PrivateProtocol {
        kind: PrivateKind::Number,
        declared_len: 1,
        field_count: 1,
        wrong_field: false,
        payload: "1e400",
    };
    let mut encoder = JsonEncoder::unlimited();
    let error = encoder
        .to_vec(&malformed_number)
        .expect_err("out-of-range number must fail");
    assert_eq!(
        error.serialization_error().map(|error| error.kind()),
        Some(qubit_json::encode::JsonSerializationErrorKind::NonFiniteFloat),
    );
}

#[test]
fn test_private_number_keeps_valid_lexeme_and_does_not_leak_invalid_text() {
    let valid = PrivateProtocol {
        kind: PrivateKind::Number,
        declared_len: 1,
        field_count: 1,
        wrong_field: false,
        payload: "1.50",
    };
    assert_eq!(
        JsonEncoder::unlimited()
            .to_vec(&valid)
            .expect("valid number lexeme should encode"),
        b"1.50",
    );

    let secret = "secret-number-marker";
    let invalid = PrivateProtocol {
        payload: secret,
        ..valid
    };
    let error = JsonEncoder::unlimited()
        .to_vec(&invalid)
        .expect_err("invalid number must fail");
    assert!(!error.to_string().contains(secret));
    assert!(!format!("{error:?}").contains(secret));
}

struct NumberNewtype(u64);

impl Serialize for NumberNewtype {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_newtype_struct("NumberNewtype", &self.0)
    }
}

struct NonTextNumberPayload;

impl Serialize for NonTextNumberPayload {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct(NUMBER, 1)?;
        state.serialize_field(NUMBER, &NumberNewtype(7))?;
        state.end()
    }
}

#[test]
fn test_private_number_rejects_non_text_newtype_payload() {
    let error = JsonEncoder::unlimited()
        .to_vec(&NonTextNumberPayload)
        .expect_err("newtype numeric payload must not bypass text validation");
    assert_eq!(
        error
            .serialization_error()
            .and_then(|error| error.serializer_state_error()),
        Some(JsonSerializerStateError::InvalidNumberProtocol),
    );
}

struct CountedMalformedNumber(Arc<AtomicUsize>);

impl Serialize for CountedMalformedNumber {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.0.fetch_add(1, Ordering::SeqCst);
        serializer.serialize_str("1e400")
    }
}

struct SwallowedNumberFailure(Arc<AtomicUsize>);

impl Serialize for SwallowedNumberFailure {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct(NUMBER, 1)?;
        let first = CountedMalformedNumber(Arc::clone(&self.0));
        let second = CountedMalformedNumber(Arc::clone(&self.0));
        let _ = state.serialize_field(NUMBER, &first);
        let _ = state.serialize_field(NUMBER, &second);
        state.end()
    }
}

#[test]
fn test_failed_number_payload_stays_failed_and_never_runs_second_callback() {
    let calls = Arc::new(AtomicUsize::new(0));
    let value = SwallowedNumberFailure(Arc::clone(&calls));
    let mut encoder = JsonEncoder::unlimited();
    let error = encoder
        .to_vec(&value)
        .expect_err("swallowed protocol failure must remain sticky");
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        error.serialization_error().map(|error| error.kind()),
        Some(qubit_json::encode::JsonSerializationErrorKind::NonFiniteFloat),
    );
}

struct CountedDisplayFailure(Arc<AtomicUsize>);

impl Display for CountedDisplayFailure {
    fn fmt(&self, _: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fetch_add(1, Ordering::SeqCst);
        Err(std::fmt::Error)
    }
}

impl Serialize for CountedDisplayFailure {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_str(self)
    }
}

struct SwallowedDisplayFailure(Arc<AtomicUsize>);

impl Serialize for SwallowedDisplayFailure {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct(NUMBER, 1)?;
        let first = CountedDisplayFailure(Arc::clone(&self.0));
        let second = CountedDisplayFailure(Arc::clone(&self.0));
        let _ = state.serialize_field(NUMBER, &first);
        let _ = state.serialize_field(NUMBER, &second);
        state.end()
    }
}

#[test]
fn test_failed_number_display_is_called_once_and_keeps_first_error() {
    let calls = Arc::new(AtomicUsize::new(0));
    let value = SwallowedDisplayFailure(Arc::clone(&calls));
    let mut encoder = JsonEncoder::unlimited();
    let error = encoder
        .to_vec(&value)
        .expect_err("failed Display callback must leave protocol failed");
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        error.serialization_error().map(|error| error.kind()),
        Some(qubit_json::encode::JsonSerializationErrorKind::DisplayFormattingFailed),
    );
}
