// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests serde_json compatibility in the budget-aware JSON serializer.

use std::fmt;

use qubit_budget::json::JsonEncodeLimits;
use qubit_budget::json::JsonEncodeSession;
use qubit_budget::json::JsonResource;
use qubit_json::text::JsonEncodeError;
use serde::Serialize;
use serde::Serializer;
use serde::ser::SerializeStruct;

use crate::text::json_encode_test_support::encode;

/// Private struct name emitted by serde_json arbitrary-precision numbers.
const JSON_NUMBER_TOKEN: &str =
    concat!("$", "serde_json", ":", ":private::Number");

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

/// Arbitrary-precision number text emitted through `collect_str`.
struct CollectedArbitraryPrecisionNumber(&'static str);

impl fmt::Display for CollectedArbitraryPrecisionNumber {
    /// Writes the configured number text to the formatter.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.0)
    }
}

impl Serialize for CollectedArbitraryPrecisionNumber {
    /// Emits serde_json's private Number shape with a collected payload.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct(JSON_NUMBER_TOKEN, 1)?;
        state.serialize_field(
            JSON_NUMBER_TOKEN,
            &CollectedNumberPayload(self),
        )?;
        state.end()
    }
}

/// Private Number field payload that delegates through `collect_str`.
struct CollectedNumberPayload<'a>(&'a CollectedArbitraryPrecisionNumber);

impl Serialize for CollectedNumberPayload<'_> {
    /// Collects the wrapped arbitrary-precision number display text.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_str(self.0)
    }
}

/// Verifies scalar serialization uses the wrapped JSON serializer.
#[test]
fn test_json_encode_serializer_serializes_scalar_values() {
    let mut session = JsonEncodeSession::owned(JsonEncodeLimits::empty());

    assert_eq!(
        encode(&true, &mut session).expect("scalar JSON should serialize"),
        b"true"
    );
}

/// Verifies a private-looking field key cannot classify a regular struct.
#[test]
fn test_json_encode_serializer_rejects_forged_private_key_as_regular_map() {
    let limits = JsonEncodeLimits::empty().with_max_map_entries(0);
    let mut session = JsonEncodeSession::owned(limits);

    let error = encode(&ForgedPrivateKey, &mut session)
        .expect_err("the ordinary struct field must consume a map entry");
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

/// Verifies private arbitrary-precision collect_str payload classification.
#[test]
fn test_json_encode_serializer_classifies_collected_private_number() {
    const NUMBER_TEXT: &str = "123456789012345678901234567890";
    let number = CollectedArbitraryPrecisionNumber(NUMBER_TEXT);
    let limits = JsonEncodeLimits::empty()
        .with_max_nodes(1)
        .with_max_map_entries(0)
        .with_max_key_bytes(0)
        .with_max_string_bytes(0)
        .with_max_number_bytes(NUMBER_TEXT.len());
    let mut session = JsonEncodeSession::owned(limits);

    let output = encode(&number, &mut session).expect(
        "private collected number should consume only the number budget",
    );

    assert_eq!(output, NUMBER_TEXT.as_bytes());
}
