// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Transactional, budget-aware projection tests.

use serde::Serialize;
use serde::ser::SerializeMap;
use serde::ser::SerializeStruct;

use crate::value::JsonValueEncoder;

struct ValidPrivateNumber;

impl Serialize for ValidPrivateNumber {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut protocol = serializer.serialize_struct(crate::internal::SERDE_JSON_NUMBER_TOKEN, 1)?;
        protocol.serialize_field(crate::internal::SERDE_JSON_NUMBER_TOKEN, "17")?;
        protocol.end()
    }
}

struct InvalidPrivateNumber;

impl Serialize for InvalidPrivateNumber {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut protocol = serializer.serialize_struct(crate::internal::SERDE_JSON_NUMBER_TOKEN, 1)?;
        protocol.serialize_field(crate::internal::SERDE_JSON_NUMBER_TOKEN, "not-a-number")?;
        protocol.end()
    }
}

struct InvalidPrivateNumberField;

impl Serialize for InvalidPrivateNumberField {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut protocol = serializer.serialize_struct(crate::internal::SERDE_JSON_NUMBER_TOKEN, 1)?;
        let _ignored = protocol.serialize_field("other", "17");
        protocol.end()
    }
}

struct IgnoresMapEndFailure;

impl Serialize for IgnoresMapEndFailure {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut map = serializer.serialize_map(Some(1))?;
        map.serialize_key("orphan")?;
        let _ignored = map.end();
        Err(serde::ser::Error::custom("later source error"))
    }
}

struct IgnoresPrivateProtocolEndFailure;

impl Serialize for IgnoresPrivateProtocolEndFailure {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let protocol = serializer.serialize_struct(crate::internal::SERDE_JSON_NUMBER_TOKEN, 1)?;
        let _ignored = protocol.end();
        Err(serde::ser::Error::custom("later source error"))
    }
}

struct MissingPrivateRawValue;

impl Serialize for MissingPrivateRawValue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let protocol = serializer.serialize_struct(crate::internal::SERDE_JSON_RAW_VALUE_TOKEN, 1)?;
        protocol.end()
    }
}

#[test]
fn test_projection_validates_real_private_number_protocol_lifecycle() {
    let projected = JsonValueEncoder::new().encode(&ValidPrivateNumber).unwrap();
    assert_eq!(projected, serde_json::json!(17));
    let error = JsonValueEncoder::new().encode(&InvalidPrivateNumber).unwrap_err();
    assert!(error.is_number_error());
}

#[test]
fn test_projection_private_protocol_child_error_is_sticky() {
    let result = JsonValueEncoder::new().encode(&InvalidPrivateNumberField);
    assert!(matches!(result, Err(error) if error.is_serializer_contract_error()));
}

#[test]
fn test_projection_keeps_ignored_compound_finalization_errors_sticky() {
    let map_error = JsonValueEncoder::new().encode(&IgnoresMapEndFailure).unwrap_err();
    assert_eq!(
        map_error.serializer_state_error(),
        Some(crate::encode::JsonSerializerStateError::MapEndedWithPendingKey)
    );

    let private_error = JsonValueEncoder::new()
        .encode(&IgnoresPrivateProtocolEndFailure)
        .unwrap_err();
    assert_eq!(
        private_error.serializer_state_error(),
        Some(crate::encode::JsonSerializerStateError::InvalidNumberProtocol)
    );
}

#[test]
fn test_projection_rejects_incomplete_private_raw_value_protocol() {
    let error = JsonValueEncoder::new().encode(&MissingPrivateRawValue).unwrap_err();
    assert!(error.is_serializer_contract_error());
    assert_eq!(
        error.serializer_state_error(),
        Some(crate::encode::JsonSerializerStateError::InvalidRawValueProtocol)
    );
}
