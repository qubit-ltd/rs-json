// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Exercises the exact RawValue and Number private serializer protocols.

use std::cell::Cell;
use std::fmt;
use std::fmt::Display;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use qubit_budget::ResourceLimit;
use qubit_budget::StructureLimits;
use qubit_budget::json::JsonEncodeLimits;
use qubit_budget::json::JsonEncodeSession;
use qubit_budget::json::JsonResource;
use qubit_budget::json::JsonValueLimits;
use qubit_json::encode::JsonEncodeError;
use qubit_json::encode::JsonEncodeErrorKind;
use qubit_json::encode::JsonEncoder;
use qubit_json::encode::JsonSerializationErrorKind;
use qubit_json::encode::JsonSerializerStateError;
use serde::Serialize;
use serde::Serializer;
use serde::ser::SerializeMap;
use serde::ser::SerializeSeq;
use serde::ser::SerializeStruct;
use serde::ser::SerializeStructVariant;
use serde::ser::SerializeTuple;
use serde::ser::SerializeTupleStruct;
use serde::ser::SerializeTupleVariant;

const RAW: &str = concat!("$", "serde_json", ":", ":private::RawValue");
const NUMBER: &str = concat!("$", "serde_json", ":", ":private::Number");

struct CountedNumberDisplay<'a> {
    calls: &'a Cell<usize>,
    chunks: usize,
}

impl fmt::Display for CountedNumberDisplay<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for _ in 0..self.chunks {
            self.calls.set(self.calls.get() + 1);
            formatter.write_str("1")?;
        }
        Ok(())
    }
}

impl Serialize for CountedNumberDisplay<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_str(self)
    }
}

struct DisplayNumber<'a>(CountedNumberDisplay<'a>);

impl Serialize for DisplayNumber<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct(NUMBER, 1)?;
        state.serialize_field(NUMBER, &self.0)?;
        state.end()
    }
}

#[derive(Clone, Copy)]
enum EmptyCompoundKind {
    Sequence,
    Tuple,
    TupleStruct,
    TupleVariant,
    Map,
    Struct,
    StructVariant,
}

struct EmptyCompound(EmptyCompoundKind, bool);

impl Serialize for EmptyCompound {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self.0 {
            EmptyCompoundKind::Sequence => {
                let mut state = serializer.serialize_seq(Some(0))?;
                if self.1 {
                    let _ = state.serialize_element(&1_u8);
                    let _ = state.serialize_element(&2_u8);
                }
                state.end()
            }
            EmptyCompoundKind::Tuple => {
                let mut state = serializer.serialize_tuple(0)?;
                if self.1 {
                    let _ = state.serialize_element(&1_u8);
                }
                state.end()
            }
            EmptyCompoundKind::TupleStruct => {
                let mut state = serializer.serialize_tuple_struct("Empty", 0)?;
                if self.1 {
                    let _ = state.serialize_field(&1_u8);
                }
                state.end()
            }
            EmptyCompoundKind::TupleVariant => {
                let mut state = serializer.serialize_tuple_variant("Empty", 0, "Variant", 0)?;
                if self.1 {
                    let _ = state.serialize_field(&1_u8);
                }
                state.end()
            }
            EmptyCompoundKind::Map => {
                let mut state = serializer.serialize_map(Some(0))?;
                if self.1 {
                    let _ = state.serialize_key("key");
                    let _ = state.serialize_value(&1_u8);
                    let _ = state.serialize_entry("entry", &2_u8);
                }
                state.end()
            }
            EmptyCompoundKind::Struct => {
                let mut state = serializer.serialize_struct("Empty", 0)?;
                if self.1 {
                    let _ = state.serialize_field("field", &1_u8);
                }
                state.skip_field("skipped")?;
                state.end()
            }
            EmptyCompoundKind::StructVariant => {
                let mut state = serializer.serialize_struct_variant("Empty", 0, "Variant", 0)?;
                if self.1 {
                    let _ = state.serialize_field("field", &1_u8);
                }
                state.skip_field("skipped")?;
                state.end()
            }
        }
    }
}

#[test]
fn test_zero_length_compounds_reject_callbacks_and_allow_empty_end() {
    let modes = [OutputMode::Vec, OutputMode::Buffered, OutputMode::Incremental];
    for kind in [
        EmptyCompoundKind::Sequence,
        EmptyCompoundKind::Tuple,
        EmptyCompoundKind::TupleStruct,
        EmptyCompoundKind::TupleVariant,
        EmptyCompoundKind::Map,
        EmptyCompoundKind::Struct,
        EmptyCompoundKind::StructVariant,
    ] {
        for finite in [false, true] {
            for mode in modes {
                let empty = EmptyCompound(kind, false);
                let (result, _) = encode(&mut encoder(finite), mode, &empty);
                result.expect("an empty compound may end without callbacks");

                let malformed = EmptyCompound(kind, true);
                let (result, _) = encode(&mut encoder(finite), mode, &malformed);
                let error = result.expect_err("callbacks after a zero-length declaration must fail");
                assert_eq!(
                    error
                        .serialization_error()
                        .and_then(|error| error.serializer_state_error()),
                    Some(JsonSerializerStateError::UnexpectedCompound),
                );
            }
        }
    }
}

/// Selects the private payload token used by protocol conformance cases.
#[derive(Clone, Copy)]
enum PrivateKind {
    Raw,
    Number,
}

impl PrivateKind {
    /// Returns the serde_json private struct token for this payload kind.
    fn token(self) -> &'static str {
        match self {
            Self::Raw => RAW,
            Self::Number => NUMBER,
        }
    }

    /// Returns a valid text payload for the selected private protocol.
    fn payload(self) -> &'static str {
        match self {
            Self::Raw => "null",
            Self::Number => "1.25",
        }
    }

    /// Returns the serializer-state error expected for a malformed payload.
    fn error(self) -> JsonSerializerStateError {
        match self {
            Self::Raw => JsonSerializerStateError::InvalidRawValueProtocol,
            Self::Number => JsonSerializerStateError::InvalidNumberProtocol,
        }
    }
}

/// Serializes a configurable private protocol shape for state-machine tests.
struct PrivateProtocol {
    kind: PrivateKind,
    declared_len: usize,
    field_count: usize,
    wrong_field: bool,
    payload: &'static str,
}

impl Serialize for PrivateProtocol {
    /// Emits the configured field count, name, and payload through Serde.
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

/// Selects the output path used to compare protocol failure behavior.
#[derive(Clone, Copy)]
enum OutputMode {
    Vec,
    Buffered,
    Incremental,
}

/// Creates an encoder with optional finite output and node limits.
fn encoder(finite: bool) -> JsonEncoder<'static, JsonResource, usize> {
    let limits = if finite {
        JsonEncodeLimits::builder()
            .output_bytes_limit(ResourceLimit::new(JsonResource::OutputBytes, 256))
            .value_limits(
                JsonValueLimits::builder()
                    .structure_limits(
                        StructureLimits::builder().nodes_limit(ResourceLimit::new(JsonResource::Nodes, 64)),
                    )
                    .build(),
            )
            .build()
    } else {
        JsonEncodeLimits::new()
    };
    JsonEncoder::new(JsonEncodeSession::from_limits(limits))
}

/// Encodes one private-protocol value and returns both result and written
/// bytes.
fn encode(
    encoder: &mut JsonEncoder<'static, JsonResource, usize>,
    mode: OutputMode,
    value: &impl Serialize,
) -> (Result<Vec<u8>, JsonEncodeError<JsonResource>>, Vec<u8>) {
    match mode {
        OutputMode::Vec => (encoder.to_vec(value), Vec::new()),
        OutputMode::Buffered => {
            let mut output = Vec::new();
            let result = encoder.write_buffered(&mut output, value).map(|()| output.clone());
            (result, output)
        }
        OutputMode::Incremental => {
            let mut output = Vec::new();
            let result = encoder.write_incremental(&mut output, value).map(|()| output.clone());
            (result, output)
        }
    }
}

#[test]
fn test_private_protocol_shape_matrix() {
    let modes = [OutputMode::Vec, OutputMode::Buffered, OutputMode::Incremental];
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
                            assert_eq!(encoder.session().value_budget().used_nodes(), finite.then_some(1));
                            assert_eq!(
                                encoder.session().output_budget().map(|budget| budget.used()),
                                finite.then_some(value.payload.len())
                            );
                        } else {
                            let error = result.expect_err("malformed private protocol must fail");
                            assert_eq!(
                                error
                                    .serialization_error()
                                    .and_then(|error| error.serializer_state_error()),
                                Some(kind.error()),
                            );
                            assert_eq!(encoder.session().value_budget().used_nodes(), finite.then_some(0));
                            let expected_output =
                                matches!(mode, OutputMode::Incremental) && fields == 2 && declared == 1;
                            assert_eq!(
                                encoder.session().output_budget().map(|budget| budget.used()),
                                finite.then_some(if expected_output { value.payload.len() } else { 0 }),
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
        let error = encoder.to_vec(&wrong_field).expect_err("wrong field must fail");
        assert_eq!(
            error
                .serialization_error()
                .and_then(|error| error.serializer_state_error()),
            Some(kind.error()),
        );
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
        Some(JsonSerializationErrorKind::NonFiniteFloat),
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

#[test]
fn test_private_number_zero_exponents_and_underflow() {
    let modes = [OutputMode::Vec, OutputMode::Buffered, OutputMode::Incremental];
    for text in [
        "0e1", "0e+10", "0.0e-10", "-0e1", "-0.0e+10", "1e-400", "-1e-400", "5e-324", "-5e-324",
    ] {
        let value = PrivateProtocol {
            kind: PrivateKind::Number,
            declared_len: 1,
            field_count: 1,
            wrong_field: false,
            payload: text,
        };
        for mode in modes {
            let mut encoder = encoder(false);
            let (result, _) = encode(&mut encoder, mode, &value);
            assert_eq!(result.expect(text), text.as_bytes(), "{text}");
        }
    }
}

/// Wraps a number in Serde newtype form to test non-text payload rejection.
struct NumberNewtype(u64);

impl Serialize for NumberNewtype {
    /// Serializes the wrapped integer as a newtype payload.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_newtype_struct("NumberNewtype", &self.0)
    }
}

/// Emits a private Number protocol whose payload is a numeric newtype.
struct NonTextNumberPayload;

impl Serialize for NonTextNumberPayload {
    /// Sends a non-text value through the private Number field.
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

/// Counts callbacks while emitting an out-of-range numeric lexeme.
struct CountedMalformedNumber(Arc<AtomicUsize>);

impl Serialize for CountedMalformedNumber {
    /// Increments the shared counter, then emits a malformed numeric string.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.0.fetch_add(1, Ordering::SeqCst);
        serializer.serialize_str("1e400")
    }
}

/// Attempts a second number callback after the first callback has failed.
struct SwallowedNumberFailure(Arc<AtomicUsize>);

impl Serialize for SwallowedNumberFailure {
    /// Suppresses both callback results to verify protocol failure stays
    /// sticky.
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
        Some(JsonSerializationErrorKind::NonFiniteFloat),
    );
}

/// Counts formatting attempts and always returns a formatting error.
struct CountedDisplayFailure(Arc<AtomicUsize>);

impl Display for CountedDisplayFailure {
    /// Records the attempt and rejects formatting the value.
    fn fmt(&self, _: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fetch_add(1, Ordering::SeqCst);
        Err(std::fmt::Error)
    }
}

impl Serialize for CountedDisplayFailure {
    /// Sends the failing display value through Serde text collection.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_str(self)
    }
}

/// Tries formatting a second display value after the first attempt fails.
struct SwallowedDisplayFailure(Arc<AtomicUsize>);

impl Serialize for SwallowedDisplayFailure {
    /// Suppresses both callback results to test sticky Display failure state.
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
        Some(JsonSerializationErrorKind::DisplayFormattingFailed),
    );
}

#[test]
fn test_number_collect_str_stops_at_number_budget() {
    let calls = Cell::new(0);
    let value = DisplayNumber(CountedNumberDisplay {
        calls: &calls,
        chunks: 10_000,
    });
    let limits = JsonEncodeLimits::<JsonResource, usize>::builder()
        .max_number_bytes(3)
        .build();
    let mut encoder = JsonEncoder::with_limits(limits);
    let error = encoder.to_vec(&value).expect_err("number must exceed budget");
    assert_eq!(error.kind(), JsonEncodeErrorKind::Budget);
    assert_eq!(
        error.budget_error().expect("budget source").resource(),
        &JsonResource::NumberBytes
    );
    assert_eq!(calls.get(), 4, "must stop at first rejected chunk");
}

#[test]
fn test_number_collect_str_stops_at_payload_budget() {
    let calls = Cell::new(0);
    let value = DisplayNumber(CountedNumberDisplay {
        calls: &calls,
        chunks: 10_000,
    });
    let limits = JsonEncodeLimits::<JsonResource, usize>::builder()
        .max_payload_bytes(3)
        .build();
    let mut encoder = JsonEncoder::with_limits(limits);
    let error = encoder.to_vec(&value).expect_err("payload must exceed budget");
    assert_eq!(error.kind(), JsonEncodeErrorKind::Budget);
    assert_eq!(
        error.budget_error().expect("budget source").resource(),
        &JsonResource::PayloadBytes
    );
    assert_eq!(calls.get(), 4, "must stop before collecting all text");
}

#[test]
fn test_number_collect_str_stops_at_depth_budget() {
    let calls = Cell::new(0);
    let value = [DisplayNumber(CountedNumberDisplay {
        calls: &calls,
        chunks: 10_000,
    })];
    let limits = JsonEncodeLimits::<JsonResource, usize>::builder().max_depth(1).build();
    let mut encoder = JsonEncoder::with_limits(limits);
    let error = encoder.to_vec(&value).expect_err("number depth must exceed budget");
    assert_eq!(error.kind(), JsonEncodeErrorKind::Budget);
    assert_eq!(
        error.budget_error().expect("budget source").resource(),
        &JsonResource::Depth
    );
    assert_eq!(calls.get(), 1, "must stop at first rejected chunk");
}

#[test]
fn test_number_collect_str_charges_once() {
    let calls = Cell::new(0);
    let value = DisplayNumber(CountedNumberDisplay {
        calls: &calls,
        chunks: 3,
    });
    let limits = JsonEncodeLimits::<JsonResource, usize>::builder()
        .max_nodes(1)
        .max_number_bytes(3)
        .max_payload_bytes(3)
        .build();
    let mut encoder = JsonEncoder::with_limits(limits);
    assert_eq!(encoder.to_vec(&value).expect("exact limits must fit"), b"111");
    assert_eq!(calls.get(), 3);
    assert_eq!(encoder.session().value_budget().used_nodes(), Some(1));
    assert_eq!(encoder.session().value_budget().used_payload_bytes(), Some(3));
}

#[test]
fn test_number_collect_str_uses_remaining_payload() {
    let limits = JsonEncodeLimits::<JsonResource, usize>::builder()
        .max_nodes(2)
        .max_payload_bytes(4)
        .build();
    let mut encoder = JsonEncoder::with_limits(limits);
    let first_calls = Cell::new(0);
    let first = DisplayNumber(CountedNumberDisplay {
        calls: &first_calls,
        chunks: 2,
    });
    assert_eq!(encoder.to_vec(&first).expect("first number fits"), b"11");
    let calls = Cell::new(0);
    let second = DisplayNumber(CountedNumberDisplay {
        calls: &calls,
        chunks: 3,
    });
    let error = encoder.to_vec(&second).expect_err("remaining payload is two bytes");
    assert_eq!(calls.get(), 3);
    assert_eq!(
        error.budget_error().expect("budget source").resource(),
        &JsonResource::PayloadBytes
    );
    assert_eq!(encoder.session().value_budget().used_payload_bytes(), Some(2));
    assert_eq!(encoder.session().value_budget().used_nodes(), Some(1));
}

#[test]
fn test_number_collect_str_preserves_output_precedence() {
    let calls = Cell::new(0);
    let value = DisplayNumber(CountedNumberDisplay {
        calls: &calls,
        chunks: 10_000,
    });
    let limits = JsonEncodeLimits::<JsonResource, usize>::builder()
        .max_output_bytes(3)
        .max_number_bytes(3)
        .build();
    let mut encoder = JsonEncoder::with_limits(limits);
    let error = encoder.to_vec(&value).expect_err("output budget is exceeded first");
    assert_eq!(
        error.budget_error().expect("budget source").resource(),
        &JsonResource::OutputBytes
    );
    assert_eq!(calls.get(), 4);
}

#[test]
fn test_number_collect_str_preserves_empty_number_error() {
    let calls = Cell::new(0);
    let value = DisplayNumber(CountedNumberDisplay {
        calls: &calls,
        chunks: 0,
    });
    let mut encoder = JsonEncoder::unlimited();
    let error = encoder.to_vec(&value).expect_err("empty text is not a number");
    assert_eq!(error.kind(), JsonEncodeErrorKind::Serialize);
    assert_eq!(
        error.serialization_error().expect("serialization source").kind(),
        JsonSerializationErrorKind::InvalidNumberRepresentation
    );
    assert_eq!(calls.get(), 0);
    assert_eq!(encoder.session().value_budget().used_nodes(), None);
}

#[test]
fn test_number_collect_str_preserves_narrow_quantity_error() {
    let calls = Cell::new(0);
    let value = DisplayNumber(CountedNumberDisplay {
        calls: &calls,
        chunks: 256,
    });
    let limits = JsonEncodeLimits::<JsonResource, u8>::builder()
        .max_number_bytes(u8::MAX)
        .build();
    let mut encoder = JsonEncoder::with_limits(limits);
    let error = encoder.to_vec(&value).expect_err("number quantity must overflow");
    assert_eq!(error.kind(), JsonEncodeErrorKind::Budget);
    assert_eq!(
        error.budget_error().expect("budget source").resource(),
        &JsonResource::NumberBytes
    );
    assert_eq!(calls.get(), 256);
}

#[test]
fn test_number_collect_str_checks_all_output_modes() {
    for mode in [OutputMode::Vec, OutputMode::Buffered, OutputMode::Incremental] {
        let calls = Cell::new(0);
        let value = [DisplayNumber(CountedNumberDisplay {
            calls: &calls,
            chunks: 10_000,
        })];
        let limits = JsonEncodeLimits::<JsonResource, usize>::builder()
            .max_nodes(8)
            .max_number_bytes(3)
            .max_output_bytes(256)
            .build();
        let mut encoder = JsonEncoder::with_limits(limits);
        let mut output = Vec::new();
        let result = match mode {
            OutputMode::Vec => encoder.to_vec(&value).map(|bytes| output = bytes),
            OutputMode::Buffered => encoder.write_buffered(&mut output, &value),
            OutputMode::Incremental => encoder.write_incremental(&mut output, &value),
        };
        let error = result.expect_err("number limit must reject every output mode");
        assert_eq!(
            error.budget_error().expect("budget source").resource(),
            &JsonResource::NumberBytes
        );
        assert_eq!(calls.get(), 4);
        assert_eq!(encoder.session().value_budget().used_nodes(), Some(0));
        match mode {
            OutputMode::Vec | OutputMode::Buffered => assert!(output.is_empty()),
            OutputMode::Incremental => assert!(!output.is_empty()),
        }
        assert_eq!(
            encoder.session().output_budget().expect("output budget").used(),
            output.len()
        );
    }
}
