// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use std::error::Error;

use qubit_json::encode::JsonCollectionKind;
use qubit_json::encode::JsonEncodeErrorKind;
use qubit_json::encode::JsonEncoder;
use qubit_json::encode::JsonIntegerSignedness;
use qubit_json::encode::JsonMapKeyKind;
use qubit_json::encode::JsonSerializationError;
use qubit_json::encode::JsonSerializationErrorCategory;
use qubit_json::encode::JsonSerializationErrorKind;
use qubit_json::encode::JsonSerializerStateError;

/// Verifies that opaque encoding failures expose their stable kind and
/// matching source without revealing their representation.
#[test]
fn test_encode_error_exposes_serialize_kind_and_source() {
    let mut encoder = JsonEncoder::unlimited();
    let error = encoder
        .to_vec(&u128::MAX)
        .expect_err("wide integer must fail strict JSON serialization");

    assert_eq!(error.kind(), JsonEncodeErrorKind::Serialize);
    assert_eq!(
        error
            .serialization_error()
            .expect("serialize error must retain its stable source")
            .kind(),
        JsonSerializationErrorKind::IntegerOutOfRange {
            signedness: JsonIntegerSignedness::Unsigned,
        },
    );
    assert!(error.budget_error().is_none());
    assert!(error.syntax_error().is_none());
    assert!(error.write_error().is_none());
    assert!(error.source().is_some());
}

/// Verifies stable encoding error kinds round-trip through their textual form.
#[test]
fn test_encode_error_kind_parses_stable_name() {
    let kind = "invalid_raw_json"
        .parse::<JsonEncodeErrorKind>()
        .expect("stable kind name must parse");

    assert_eq!(kind, JsonEncodeErrorKind::InvalidRawJson);
    assert_eq!(kind.to_string(), "invalid_raw_json");
}

/// Verifies Serde custom failures discard arbitrary diagnostic text.
#[test]
fn test_serde_custom_error_redacts_message() {
    const SECRET: &str = "SERIALIZATION_SECRET";
    let error = <JsonSerializationError as serde::ser::Error>::custom(SECRET);

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
