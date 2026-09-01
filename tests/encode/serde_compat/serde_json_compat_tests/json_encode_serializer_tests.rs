// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests serde_json compatibility in the budget-aware JSON serializer.

use qubit_budget::json::JsonEncodeLimits;
use qubit_budget::json::JsonEncodeSession;
use qubit_budget::json::JsonResource;
use qubit_json::encode::JsonEncodeError;
use serde::Serialize;
use serde::Serializer;
use serde::ser::SerializeStruct;

use crate::encode::json_encode_test_support::encode;

/// Former private struct name emitted by serde_json arbitrary-precision
/// numbers.
const JSON_NUMBER_TOKEN: &str = concat!("$", "serde_json", ":", ":private::Number");

/// A regular struct whose field name resembles serde_json private metadata.
struct ForgedPrivateKey;

impl Serialize for ForgedPrivateKey {
    /// Emits an ordinary struct with a private-looking field key.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("ForgedPrivateKey", 1)?;
        state.serialize_field(JSON_NUMBER_TOKEN, &true)?;
        state.end()
    }
}

/// Verifies scalar serialization uses the wrapped JSON serializer.
#[test]
fn test_json_encode_serializer_serializes_scalar_values() {
    let mut session = JsonEncodeSession::from_limits(JsonEncodeLimits::<JsonResource, usize>::builder().build());

    assert_eq!(
        encode(&true, &mut session).expect("scalar JSON should serialize"),
        b"true"
    );
}

/// Verifies a private-looking field key cannot classify a regular struct.
#[test]
fn test_json_encode_serializer_rejects_forged_private_key_as_regular_map() {
    let limits = JsonEncodeLimits::<JsonResource, usize>::builder()
        .max_map_entries(0)
        .build();
    let mut session = JsonEncodeSession::from_limits(limits);

    let error =
        encode(&ForgedPrivateKey, &mut session).expect_err("the ordinary struct field must consume a map entry");
    let JsonEncodeError::Budget(error) = error else {
        panic!("expected a budget error, got {error:?}");
    };

    assert_eq!(
        error
            .budget_error()
            .expect("the error must retain its budget failure")
            .resource(),
        &JsonResource::MapEntries,
    );
}
