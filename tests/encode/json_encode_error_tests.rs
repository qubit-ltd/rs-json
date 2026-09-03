// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use std::error::Error;
use std::io;
use std::io::Write;

use qubit_budget::json::JsonEncodeLimits;
use qubit_budget::json::JsonResource;
use qubit_json::encode::JsonCollectionKind;
use qubit_json::encode::JsonEncodeError;
use qubit_json::encode::JsonEncodeErrorKind;
use qubit_json::encode::JsonEncoder;
use qubit_json::encode::JsonIntegerSignedness;
use qubit_json::encode::JsonMapKeyKind;
use qubit_json::encode::JsonSerializationError;
use qubit_json::encode::JsonSerializationErrorCategory;
use qubit_json::encode::JsonSerializationErrorKind;
use qubit_json::encode::JsonSerializerStateError;
use serde::Serialize;
use serde::Serializer;
use serde::ser::Error as SerdeError;
use serde::ser::SerializeStruct;

/// Simulates serde_json's private raw-value protocol with invalid JSON text.
struct InvalidRawValue;

impl Serialize for InvalidRawValue {
    /// Emits an unterminated array through the private raw-value protocol.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct(concat!("$", "serde_json", ":", ":private::RawValue"), 1)?;
        state.serialize_field(concat!("$", "serde_json", ":", ":private::RawValue"), "[")?;
        state.end()
    }
}

/// Destination that rejects every non-empty output write.
struct RejectingWriter;

impl Write for RejectingWriter {
    /// Rejects the supplied output bytes with a stable error kind.
    fn write(&mut self, _buffer: &[u8]) -> io::Result<usize> {
        Err(io::Error::other("destination rejected JSON"))
    }

    /// Accepts flushing because encoding must fail during the write itself.
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Produces a public output-budget encoding failure.
fn create_budget_error() -> JsonEncodeError<JsonResource> {
    let limits = JsonEncodeLimits::builder().max_output_bytes(0).build();
    let mut encoder = JsonEncoder::with_limits(limits);
    encoder
        .to_vec(&true)
        .expect_err("zero output budget must reject a boolean")
}

/// Produces a public invalid-raw-JSON encoding failure.
fn create_invalid_raw_json_error() -> JsonEncodeError<JsonResource> {
    let mut encoder = JsonEncoder::unlimited();
    encoder
        .to_vec(&InvalidRawValue)
        .expect_err("unterminated raw JSON must fail strict encoding")
}

/// Produces a public Serde serialization failure.
fn create_serialization_error() -> JsonEncodeError<JsonResource> {
    let mut encoder = JsonEncoder::unlimited();
    encoder
        .to_vec(&u128::MAX)
        .expect_err("wide integer must fail strict JSON serialization")
}

/// Produces a public destination-writer failure.
fn create_write_error() -> JsonEncodeError<JsonResource> {
    let mut encoder = JsonEncoder::unlimited();
    encoder
        .write_buffered(RejectingWriter, &true)
        .expect_err("rejecting destination must fail buffered output")
}

/// Verifies budget failures expose only their matching public payload.
#[test]
fn test_encode_error_exposes_budget_contract() {
    let error = create_budget_error();

    assert_eq!(error.kind(), JsonEncodeErrorKind::Budget);
    assert!(error.budget_error().is_some());
    assert!(error.syntax_error().is_none());
    assert!(error.serialization_error().is_none());
    assert!(error.write_error().is_none());
    assert!(error.source().is_some());
    assert_eq!(
        error.to_string(),
        error.source().expect("budget source must exist").to_string()
    );

    assert!(create_budget_error().into_budget_error().is_some());
    assert!(create_invalid_raw_json_error().into_budget_error().is_none());
    assert!(create_serialization_error().into_budget_error().is_none());
    assert!(create_write_error().into_budget_error().is_none());
}

/// Verifies invalid raw JSON exposes only its syntax payload.
#[test]
fn test_encode_error_exposes_invalid_raw_json_contract() {
    let error = create_invalid_raw_json_error();

    assert_eq!(error.kind(), JsonEncodeErrorKind::InvalidRawJson);
    assert!(error.budget_error().is_none());
    assert!(error.syntax_error().is_some());
    assert!(error.serialization_error().is_none());
    assert!(error.write_error().is_none());
    assert!(error.source().is_some());
    assert!(error.to_string().starts_with("JSON raw value is invalid: "));

    assert!(create_budget_error().into_syntax_error().is_none());
    assert!(create_invalid_raw_json_error().into_syntax_error().is_some());
    assert!(create_serialization_error().into_syntax_error().is_none());
    assert!(create_write_error().into_syntax_error().is_none());
}

/// Verifies Serde failures expose only their privacy-safe payload.
#[test]
fn test_encode_error_exposes_serialization_contract() {
    let error = create_serialization_error();

    assert_eq!(error.kind(), JsonEncodeErrorKind::Serialize);
    assert!(error.budget_error().is_none());
    assert!(error.syntax_error().is_none());
    assert_eq!(
        error
            .serialization_error()
            .expect("serialize error must retain its stable source")
            .kind(),
        JsonSerializationErrorKind::IntegerOutOfRange {
            signedness: JsonIntegerSignedness::Unsigned,
        },
    );
    assert!(error.write_error().is_none());
    assert!(error.source().is_some());
    assert!(error.to_string().starts_with("JSON serialization failed: "));

    assert!(create_budget_error().into_serialization_error().is_none());
    assert!(create_invalid_raw_json_error().into_serialization_error().is_none());
    assert!(create_serialization_error().into_serialization_error().is_some());
    assert!(create_write_error().into_serialization_error().is_none());
}

/// Verifies destination failures expose only their I/O payload.
#[test]
fn test_encode_error_exposes_write_contract() {
    let error = create_write_error();

    assert_eq!(error.kind(), JsonEncodeErrorKind::Write);
    assert!(error.budget_error().is_none());
    assert!(error.syntax_error().is_none());
    assert!(error.serialization_error().is_none());
    assert_eq!(
        error
            .write_error()
            .expect("write error must retain its I/O source")
            .kind(),
        io::ErrorKind::Other,
    );
    assert!(error.source().is_some());
    assert!(error.to_string().starts_with("JSON output writer failed: "));

    assert!(create_budget_error().into_write_error().is_none());
    assert!(create_invalid_raw_json_error().into_write_error().is_none());
    assert!(create_serialization_error().into_write_error().is_none());
    assert_eq!(
        create_write_error()
            .into_write_error()
            .expect("write error must be extractable")
            .kind(),
        io::ErrorKind::Other,
    );
}

/// Verifies measured-budget failures convert into the stable encode error.
#[test]
fn test_encode_error_converts_measured_budget_error() {
    let source = create_budget_error()
        .into_budget_error()
        .expect("budget failure must expose its measured source");
    let error: JsonEncodeError<JsonResource> = source.into();

    assert_eq!(error.kind(), JsonEncodeErrorKind::Budget);
    assert!(error.budget_error().is_some());
}

/// Verifies every stable encoding kind round-trips case-insensitively.
#[test]
fn test_encode_error_kind_round_trips_stable_names() {
    let cases = [
        ("budget", JsonEncodeErrorKind::Budget),
        ("invalid_raw_json", JsonEncodeErrorKind::InvalidRawJson),
        ("serialize", JsonEncodeErrorKind::Serialize),
        ("write", JsonEncodeErrorKind::Write),
    ];

    for (name, kind) in cases {
        assert_eq!(name.parse::<JsonEncodeErrorKind>(), Ok(kind));
        assert_eq!(name.to_ascii_uppercase().parse::<JsonEncodeErrorKind>(), Ok(kind));
        assert_eq!(kind.to_string(), name);
    }

    assert_eq!(
        "unknown".parse::<JsonEncodeErrorKind>(),
        Err("unknown JsonEncodeErrorKind"),
    );
}

/// Verifies Serde custom failures discard arbitrary diagnostic text.
#[test]
fn test_serde_custom_error_redacts_message() {
    const SECRET: &str = "SERIALIZATION_SECRET";
    let error = <JsonSerializationError as SerdeError>::custom(SECRET);

    assert_eq!(error.kind(), JsonSerializationErrorKind::CustomSerialization);
    assert_eq!(error.category(), JsonSerializationErrorCategory::Custom);
    assert!(!error.to_string().contains(SECRET));
    assert!(!format!("{error:?}").contains(SECRET));
    assert!(error.source().is_none());
}

/// Verifies optional serialization-error accessors return their exact payload
/// only for the matching stable kind.
#[test]
fn test_serialization_error_accessors_match_stable_kinds() {
    let integer = JsonSerializationError::new(JsonSerializationErrorKind::IntegerOutOfRange {
        signedness: JsonIntegerSignedness::Signed,
    });
    assert_eq!(integer.integer_signedness(), Some(JsonIntegerSignedness::Signed));
    assert_eq!(integer.map_key_kind(), None);
    assert!(integer.is_number_error());

    let map_key = JsonSerializationError::new(JsonSerializationErrorKind::UnsupportedMapKey {
        kind: JsonMapKeyKind::Map,
    });
    assert_eq!(map_key.map_key_kind(), Some(JsonMapKeyKind::Map));
    assert_eq!(map_key.collection_kind(), None);
    assert!(map_key.is_map_key_error());

    let collection = JsonSerializationError::new(JsonSerializationErrorKind::CollectionLengthOverflow {
        kind: JsonCollectionKind::Array,
    });
    assert_eq!(collection.collection_kind(), Some(JsonCollectionKind::Array));
    assert_eq!(collection.serializer_state_error(), None);

    let state = JsonSerializationError::new(JsonSerializationErrorKind::InvalidSerializerState {
        reason: JsonSerializerStateError::MapValueWithoutKey,
    });
    assert_eq!(
        state.serializer_state_error(),
        Some(JsonSerializerStateError::MapValueWithoutKey),
    );
    assert!(state.is_serializer_contract_error());

    let raw = JsonSerializationError::new(JsonSerializationErrorKind::InvalidRawValue);
    assert!(raw.is_raw_value_error());
    assert_eq!(raw.integer_signedness(), None);
}
