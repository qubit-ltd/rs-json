// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests for the shared serde_json private-protocol state machine.

use crate::encode::JsonSerializerStateError;
use crate::internal::JsonPrivateProtocolKind;
use crate::internal::JsonPrivateProtocolState;
use crate::internal::SERDE_JSON_NUMBER_TOKEN;
use crate::internal::SERDE_JSON_RAW_VALUE_TOKEN;

fn success(kind: JsonPrivateProtocolKind) {
    let mut state = JsonPrivateProtocolState::new(kind, 1).expect("one field is valid");
    state.begin(kind.field_name()).expect("matching field begins payload");
    state.complete().expect("payload completes");
    assert_eq!(state.finish(), Ok(()));
}

#[test]
fn test_protocol_success_for_each_kind() {
    success(JsonPrivateProtocolKind::RawValue);
    success(JsonPrivateProtocolKind::Number);
    assert_eq!(SERDE_JSON_RAW_VALUE_TOKEN, "$serde_json::private::RawValue");
    assert_eq!(SERDE_JSON_NUMBER_TOKEN, "$serde_json::private::Number");
    assert_eq!(
        JsonPrivateProtocolKind::from_struct_name(SERDE_JSON_RAW_VALUE_TOKEN),
        Some(JsonPrivateProtocolKind::RawValue)
    );
    assert_eq!(
        JsonPrivateProtocolKind::from_struct_name(SERDE_JSON_NUMBER_TOKEN),
        Some(JsonPrivateProtocolKind::Number)
    );
    assert_eq!(JsonPrivateProtocolKind::from_struct_name("other"), None);
}

#[test]
fn test_protocol_rejects_invalid_declared_lengths() {
    assert_eq!(
        JsonPrivateProtocolState::new(JsonPrivateProtocolKind::RawValue, 0),
        Err(JsonSerializerStateError::InvalidRawValueProtocol),
    );
    assert_eq!(
        JsonPrivateProtocolState::new(JsonPrivateProtocolKind::Number, 2),
        Err(JsonSerializerStateError::InvalidNumberProtocol),
    );
}

#[test]
fn test_protocol_rejects_empty_second_wrong_and_in_flight_end() {
    let empty = JsonPrivateProtocolState::new(JsonPrivateProtocolKind::Number, 1).expect("one field is valid");
    assert_eq!(empty.finish(), Err(JsonSerializerStateError::InvalidNumberProtocol));

    let mut wrong = JsonPrivateProtocolState::new(JsonPrivateProtocolKind::RawValue, 1).expect("one field is valid");
    assert_eq!(
        wrong.begin("other"),
        Err(JsonSerializerStateError::InvalidRawValueProtocol)
    );
    assert_eq!(wrong.finish(), Err(JsonSerializerStateError::InvalidRawValueProtocol));

    let mut second = JsonPrivateProtocolState::new(JsonPrivateProtocolKind::Number, 1).expect("one field is valid");
    second
        .begin(SERDE_JSON_NUMBER_TOKEN)
        .expect("the number payload field begins");
    second.complete().expect("the number payload completes");
    assert_eq!(
        second.begin(SERDE_JSON_NUMBER_TOKEN),
        Err(JsonSerializerStateError::InvalidNumberProtocol)
    );
    assert_eq!(second.finish(), Err(JsonSerializerStateError::InvalidNumberProtocol));

    let mut in_flight =
        JsonPrivateProtocolState::new(JsonPrivateProtocolKind::RawValue, 1).expect("one field is valid");
    in_flight
        .begin(SERDE_JSON_RAW_VALUE_TOKEN)
        .expect("the raw value payload field begins");
    assert_eq!(
        in_flight.finish(),
        Err(JsonSerializerStateError::InvalidRawValueProtocol)
    );
}

#[test]
fn test_protocol_failure_is_sticky() {
    let mut state = JsonPrivateProtocolState::new(JsonPrivateProtocolKind::Number, 1).expect("one field is valid");
    state.fail();
    assert_eq!(state.finish(), Err(JsonSerializerStateError::InvalidNumberProtocol));
}
