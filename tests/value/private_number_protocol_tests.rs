// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Verifies private Number protocol handling in the Value encoder.

#[path = "support/mod.rs"]
mod support;

use std::collections::BTreeMap;

use qubit_json::encode::JsonSerializationErrorKind;
use qubit_json::value::JsonValueEncoder;
use serde::Serialize;
use serde::Serializer;
use serde::ser::SerializeStruct;
use serde_json::Value;

const NUMBER: &str = concat!("$", "serde_json", ":", ":private::Number");
const RAW: &str = concat!("$", "serde_json", ":", ":private::RawValue");

#[derive(Clone, Copy)]
struct NumberProtocol {
    declared_len: usize,
    fields: usize,
    field: &'static str,
    payload: &'static str,
}

impl Serialize for NumberProtocol {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct(NUMBER, self.declared_len)?;
        for _ in 0..self.fields {
            state.serialize_field(self.field, self.payload)?;
        }
        state.end()
    }
}

#[test]
fn test_private_number_protocol_constructs_only_a_number() {
    let valid = NumberProtocol {
        declared_len: 1,
        fields: 1,
        field: NUMBER,
        payload: "18446744073709551615",
    };
    assert_eq!(
        JsonValueEncoder::new().encode(&valid),
        Ok(Value::Number(serde_json::Number::from(u64::MAX))),
    );
    assert_eq!(
        JsonValueEncoder::new().encode(&serde_json::Number::from(42)),
        Ok(serde_json::json!(42)),
    );

    for invalid in ["1e400", "01", "{}"] {
        let value = NumberProtocol {
            payload: invalid,
            ..valid
        };
        let error = JsonValueEncoder::new()
            .encode(&value)
            .expect_err("invalid number text must fail");
        assert!(matches!(
            error.kind(),
            JsonSerializationErrorKind::NonFiniteFloat
                | JsonSerializationErrorKind::InvalidNumberRepresentation
        ));
    }
}

#[test]
fn test_private_number_protocol_rejects_wrong_shapes_and_keeps_lookalike_map_keys() {
    let wrong_field = NumberProtocol {
        field: RAW,
        ..valid_probe()
    };
    let error = JsonValueEncoder::new()
        .encode(&wrong_field)
        .expect_err("wrong field must fail");
    support::assert_number_protocol_error(&error);

    let no_field = NumberProtocol {
        fields: 0,
        ..valid_probe()
    };
    let error = JsonValueEncoder::new()
        .encode(&no_field)
        .expect_err("missing field must fail");
    support::assert_number_protocol_error(&error);

    let multiple_fields = NumberProtocol {
        fields: 2,
        ..valid_probe()
    };
    let error = JsonValueEncoder::new()
        .encode(&multiple_fields)
        .expect_err("multiple fields must fail");
    support::assert_number_protocol_error(&error);

    let wrong_length = NumberProtocol {
        declared_len: 2,
        ..valid_probe()
    };
    let error = JsonValueEncoder::new()
        .encode(&wrong_length)
        .expect_err("wrong declared length must fail");
    support::assert_number_protocol_error(&error);

    let mut object = BTreeMap::new();
    object.insert(NUMBER.to_owned(), 1);
    assert_eq!(
        JsonValueEncoder::new().encode(&object),
        Ok(serde_json::json!({ NUMBER: 1 })),
    );
}

struct NonTextNumberPayload;

impl Serialize for NonTextNumberPayload {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct(NUMBER, 1)?;
        state.serialize_field(NUMBER, &7_u64)?;
        state.end()
    }
}

#[test]
fn test_private_number_value_rejects_non_text_payload() {
    let error = JsonValueEncoder::new()
        .encode(&NonTextNumberPayload)
        .expect_err("numeric payload must be carried as text");
    support::assert_number_protocol_error(&error);
}

fn valid_probe() -> NumberProtocol {
    NumberProtocol {
        declared_len: 1,
        fields: 1,
        field: NUMBER,
        payload: "1.5",
    }
}
