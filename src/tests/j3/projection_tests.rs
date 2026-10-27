// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Transactional, budget-aware projection tests.

use qubit_budget::ResourceLimit;
use qubit_budget::StructureLimits;
use qubit_budget::json::JsonResource;
use qubit_budget::json::JsonValueBudget;
use qubit_budget::json::JsonValueLimits;
use serde::Deserialize;
use serde::Serialize;
use serde::ser::SerializeMap;
use serde::ser::SerializeSeq;
use serde::ser::SerializeStruct;

use crate::value::JsonValueEncoder;

#[test]
fn test_projection_stages_multiple_values_and_commits_once() {
    let mut budget = JsonValueBudget::new(
        JsonValueLimits::<OpaqueResource, u64>::builder()
            .payload_bytes_limit(ResourceLimit::new(OpaqueResource, 8_u64))
            .structure_limits(
                StructureLimits::<OpaqueResource, u64>::builder()
                    .nodes_limit(ResourceLimit::new(OpaqueResource, 3_u64))
                    .build(),
            )
            .build(),
    );
    let mut transaction = budget.transaction();
    let encoder = JsonValueEncoder::new();
    let first = encoder
        .encode_in_transaction(&vec![0_u8], &mut transaction)
        .unwrap_or_else(|_| panic!("first projection must fit"));
    let second = encoder
        .encode_in_transaction(&true, &mut transaction)
        .unwrap_or_else(|_| panic!("second projection must fit"));
    assert_eq!(transaction.used_nodes(), Some(3_u64));
    transaction
        .commit()
        .unwrap_or_else(|_| panic!("staged projections must commit"));
    assert_eq!(budget.used_nodes(), Some(3_u64));
    drop((first, second));
}

#[test]
fn test_projection_failed_batch_is_not_published() {
    let mut budget = JsonValueBudget::new(
        JsonValueLimits::<JsonResource, usize>::builder()
            .max_nodes(1_usize)
            .build(),
    );
    {
        let mut transaction = budget.transaction();
        let encoder = JsonValueEncoder::new();
        let _first = encoder.encode_in_transaction(&true, &mut transaction).unwrap();
        assert!(encoder.encode_in_transaction(&false, &mut transaction).is_err());
    }
    assert_eq!(budget.used_nodes(), Some(0));
}

#[derive(Clone)]
struct OpaqueResource;

#[test]
fn test_projection_accepts_non_debug_resource_and_u64_quantity() {
    let limits = JsonValueLimits::<OpaqueResource, u64>::builder()
        .structure_limits(
            StructureLimits::<OpaqueResource, u64>::builder()
                .nodes_limit(ResourceLimit::new(OpaqueResource, 0_u64))
                .build(),
        )
        .build();
    let mut budget = JsonValueBudget::new(limits);
    let mut transaction = budget.transaction();
    let encoder = JsonValueEncoder::new();
    let result = encoder.encode_in_transaction(&true, &mut transaction);
    assert!(matches!(result, Err(crate::value::JsonValueEncodeError::Budget(_))));
}

#[test]
fn test_projection_opaque_resource_supports_serde_compound_shapes() {
    #[derive(Serialize)]
    struct Unit;
    #[derive(Serialize)]
    struct Newtype(i32);
    #[derive(Serialize)]
    struct Tuple(i32, bool);
    #[derive(Serialize)]
    struct Record {
        name: &'static str,
        active: bool,
    }
    #[derive(Serialize)]
    enum Variant {
        Unit,
        Newtype(i32),
        Tuple(i32, bool),
        Record { name: &'static str },
    }

    let mut budget = JsonValueBudget::new(JsonValueLimits::<OpaqueResource, u64>::new());
    let mut transaction = budget.transaction();
    let encoder = JsonValueEncoder::new();
    let values = [
        encoder
            .encode_in_transaction(&Unit, &mut transaction)
            .unwrap_or_else(|_| panic!("unit struct projection must succeed")),
        encoder
            .encode_in_transaction(&Newtype(3), &mut transaction)
            .unwrap_or_else(|_| panic!("newtype struct projection must succeed")),
        encoder
            .encode_in_transaction(&Tuple(4, true), &mut transaction)
            .unwrap_or_else(|_| panic!("tuple struct projection must succeed")),
        encoder
            .encode_in_transaction(
                &Record {
                    name: "record",
                    active: true,
                },
                &mut transaction,
            )
            .unwrap_or_else(|_| panic!("struct projection must succeed")),
        encoder
            .encode_in_transaction(&Variant::Unit, &mut transaction)
            .unwrap_or_else(|_| panic!("unit variant projection must succeed")),
        encoder
            .encode_in_transaction(&Variant::Newtype(5), &mut transaction)
            .unwrap_or_else(|_| panic!("newtype variant projection must succeed")),
        encoder
            .encode_in_transaction(&Variant::Tuple(6, false), &mut transaction)
            .unwrap_or_else(|_| panic!("tuple variant projection must succeed")),
        encoder
            .encode_in_transaction(&Variant::Record { name: "variant" }, &mut transaction)
            .unwrap_or_else(|_| panic!("struct variant projection must succeed")),
    ];
    assert_eq!(values[0].as_value(), &serde_json::Value::Null);
    assert_eq!(values[1].as_value(), &serde_json::json!(3));
    assert_eq!(values[2].as_value(), &serde_json::json!([4, true]));
    assert_eq!(
        values[3].as_value(),
        &serde_json::json!({"name": "record", "active": true})
    );
    assert_eq!(values[4].as_value(), &serde_json::json!("Unit"));
    assert_eq!(values[5].as_value(), &serde_json::json!({"Newtype": 5}));
    assert_eq!(values[6].as_value(), &serde_json::json!({"Tuple": [6, false]}));
    assert_eq!(
        values[7].as_value(),
        &serde_json::json!({"Record": {"name": "variant"}})
    );
    transaction
        .commit()
        .unwrap_or_else(|_| panic!("opaque resource transaction must commit"));
}

struct IgnoresChildSerializationError;

impl Serialize for IgnoresChildSerializationError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut sequence = serializer.serialize_seq(Some(2))?;
        let _ignored = sequence.serialize_element(&i128::MAX);
        sequence.end()
    }
}

#[test]
fn test_projection_keeps_first_error_when_source_ignores_child_error() {
    let mut budget = JsonValueBudget::new(JsonValueLimits::<JsonResource, usize>::new());
    let mut transaction = budget.transaction();
    let result = JsonValueEncoder::new().encode_in_transaction(&IgnoresChildSerializationError, &mut transaction);
    assert!(matches!(
        result,
        Err(crate::value::JsonValueEncodeError::Serialization(error))
            if error.integer_signedness() == Some(crate::encode::JsonIntegerSignedness::Signed)
    ));
}

#[test]
fn test_projection_keeps_budget_failure_when_compound_ignores_child_error() {
    struct IgnoreBudgetChild;
    impl Serialize for IgnoreBudgetChild {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            let mut sequence = serializer.serialize_seq(Some(1))?;
            let _ignored = sequence.serialize_element(&true);
            sequence.end()
        }
    }

    let limits = JsonValueLimits::<JsonResource, usize>::builder()
        .max_nodes(1_usize)
        .build();
    let mut budget = JsonValueBudget::new(limits);
    {
        let mut transaction = budget.transaction();
        let result = JsonValueEncoder::new().encode_in_transaction(&IgnoreBudgetChild, &mut transaction);
        assert!(matches!(result, Err(crate::value::JsonValueEncodeError::Budget(_))));
    }
    assert_eq!(budget.used_nodes(), Some(0));
}

struct PanicsAfterDeepChild;

impl Serialize for PanicsAfterDeepChild {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut deep = serde_json::Value::Null;
        for _ in 0..128 {
            deep = serde_json::Value::Array(vec![deep]);
        }
        let mut sequence = serializer.serialize_seq(Some(2))?;
        sequence.serialize_element(&deep)?;
        panic!("source serializer panic after a deep child");
    }
}

#[test]
fn test_projection_drops_partial_tree_when_source_panics() {
    let mut budget = JsonValueBudget::new(JsonValueLimits::<JsonResource, usize>::builder().max_nodes(200).build());
    {
        let mut transaction = budget.transaction();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            JsonValueEncoder::new().encode_in_transaction(&PanicsAfterDeepChild, &mut transaction)
        }));
        assert!(result.is_err());
    }
    assert_eq!(budget.used_nodes(), Some(0));
}

struct LaterSerializationError;

impl Serialize for LaterSerializationError {
    fn serialize<S>(&self, _: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        Err(serde::ser::Error::custom("later failure"))
    }
}

struct SerializesChildThenFails;

impl Serialize for SerializesChildThenFails {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut deep = serde_json::Value::Null;
        for _ in 0..128 {
            deep = serde_json::Value::Array(vec![deep]);
        }
        let mut sequence = serializer.serialize_seq(Some(2))?;
        sequence.serialize_element(&deep)?;
        sequence.serialize_element(&LaterSerializationError)?;
        sequence.end()
    }
}

#[test]
fn test_projection_late_serialization_error_discards_staged_transaction() {
    let limits = JsonValueLimits::<JsonResource, usize>::builder()
        .max_nodes(200_usize)
        .build();
    let mut budget = JsonValueBudget::new(limits);
    {
        let mut transaction = budget.transaction();
        let result = JsonValueEncoder::new().encode_in_transaction(&SerializesChildThenFails, &mut transaction);
        assert!(matches!(
            result,
            Err(crate::value::JsonValueEncodeError::Serialization(_))
        ));
    }
    assert_eq!(budget.used_nodes(), Some(0));
}

struct HugeLengthHint;

impl Serialize for HugeLengthHint {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut sequence = serializer.serialize_seq(Some(usize::MAX))?;
        sequence.serialize_element(&true)?;
        sequence.end()
    }
}

#[test]
fn test_projection_clamps_untrusted_length_hint() {
    let projected = JsonValueEncoder::new().encode(&HugeLengthHint).unwrap();
    assert_eq!(projected, serde_json::json!([true]));
}

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

#[test]
fn test_projection_validates_real_private_number_protocol_lifecycle() {
    let projected = JsonValueEncoder::new().encode(&ValidPrivateNumber).unwrap();
    assert_eq!(projected, serde_json::json!(17));
    let error = JsonValueEncoder::new().encode(&InvalidPrivateNumber).unwrap_err();
    assert!(error.is_number_error());
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

#[test]
fn test_projection_private_protocol_child_error_is_sticky() {
    let result = JsonValueEncoder::new().encode(&InvalidPrivateNumberField);
    assert!(matches!(result, Err(error) if error.is_serializer_contract_error()));
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
fn test_projection_raw_value_uses_transaction_and_no_synthetic_root_charge() {
    let mut deserializer = serde_json::Deserializer::from_str("[0]");
    let raw: &serde_json::value::RawValue = <&serde_json::value::RawValue>::deserialize(&mut deserializer).unwrap();
    let mut budget = JsonValueBudget::new(
        JsonValueLimits::<JsonResource, usize>::builder()
            .max_nodes(2)
            .max_depth(2)
            .build(),
    );
    let mut transaction = budget.transaction();
    let projected = JsonValueEncoder::new()
        .encode_in_transaction(raw, &mut transaction)
        .unwrap();
    assert_eq!(projected.as_value(), &serde_json::json!([0]));
    assert_eq!(transaction.used_nodes(), Some(2));
}

#[test]
fn test_projection_raw_value_inherits_the_actual_container_depth() {
    let mut deserializer = serde_json::Deserializer::from_str("[0]");
    let raw: &serde_json::value::RawValue = <&serde_json::value::RawValue>::deserialize(&mut deserializer).unwrap();
    let mut budget = JsonValueBudget::new(JsonValueLimits::<JsonResource, usize>::builder().max_depth(2).build());
    let mut transaction = budget.transaction();
    let result = JsonValueEncoder::new().encode_in_transaction(&vec![raw], &mut transaction);
    assert!(result.is_err());
}

#[test]
fn test_projection_raw_value_accepts_129_container_levels() {
    let text = format!("{}0{}", "[".repeat(129), "]".repeat(129));
    let mut deserializer = serde_json::Deserializer::from_str(&text);
    deserializer.disable_recursion_limit();
    let raw: &serde_json::value::RawValue = <&serde_json::value::RawValue>::deserialize(&mut deserializer).unwrap();
    let mut budget = JsonValueBudget::new(JsonValueLimits::<JsonResource, usize>::new());
    let mut transaction = budget.transaction();
    let projected = JsonValueEncoder::new().encode_in_transaction(raw, &mut transaction);
    assert!(projected.is_ok());
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
fn test_projection_rejects_incomplete_private_raw_value_protocol() {
    let error = JsonValueEncoder::new().encode(&MissingPrivateRawValue).unwrap_err();
    assert!(error.is_serializer_contract_error());
    assert_eq!(
        error.serializer_state_error(),
        Some(crate::encode::JsonSerializerStateError::InvalidRawValueProtocol)
    );
}

#[test]
fn test_legacy_encode_deep_result_can_be_immediately_taken_into_safe_owner() {
    let mut source = serde_json::Value::Null;
    for _ in 0..129 {
        source = serde_json::Value::Array(vec![source]);
    }
    let source = crate::value::SafeJsonValue::new(source);
    let projected = JsonValueEncoder::new().encode(source.as_value()).unwrap();
    let projected = crate::value::SafeJsonValue::new(projected);
    assert!(projected.as_value().is_array());
    drop(projected);
    drop(source);
}

#[test]
fn test_projection_charges_utf8_payload_bytes_exactly() {
    let limits = JsonValueLimits::<JsonResource, usize>::builder()
        .max_nodes(1_usize)
        .max_payload_bytes(2_usize)
        .build();
    let mut budget = JsonValueBudget::new(limits);
    let mut transaction = budget.transaction();
    let projected = JsonValueEncoder::new().encode_in_transaction("é", &mut transaction);
    assert!(projected.is_ok());
    assert_eq!(transaction.used_nodes(), Some(1));
    assert_eq!(transaction.used_payload_bytes(), Some(2));
}

struct ByteSlice;

impl Serialize for ByteSlice {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_bytes(&[1, 255])
    }
}

#[test]
fn test_projection_charges_bytes_as_json_array_elements() {
    let limits = JsonValueLimits::<JsonResource, usize>::builder()
        .max_nodes(3)
        .max_payload_bytes(4)
        .build();
    let mut budget = JsonValueBudget::new(limits);
    let mut transaction = budget.transaction();
    let projected = JsonValueEncoder::new()
        .encode_in_transaction(&ByteSlice, &mut transaction)
        .unwrap();
    assert_eq!(projected.as_value(), &serde_json::json!([1, 255]));
    assert_eq!(transaction.used_nodes(), Some(3));
    assert_eq!(transaction.used_payload_bytes(), Some(4));
}

#[test]
fn test_projection_literal_private_protocol_keys_remain_objects() {
    let value = std::collections::BTreeMap::from([
        ("$serde_json::private::RawValue", "[0]"),
        ("$serde_json::private::Number", "7"),
    ]);
    let projected = JsonValueEncoder::new().encode(&value).unwrap();
    assert_eq!(
        projected,
        serde_json::json!({
            "$serde_json::private::RawValue": "[0]",
            "$serde_json::private::Number": "7",
        })
    );
}

struct UnsupportedSequenceMapKey;

impl Serialize for UnsupportedSequenceMapKey {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut map = serializer.serialize_map(Some(1))?;
        map.serialize_entry(&[1_u8, 2_u8], &true)?;
        map.end()
    }
}

struct DuplicateMapKey;

impl Serialize for DuplicateMapKey {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut map = serializer.serialize_map(Some(2))?;
        map.serialize_entry("private-key-42", &1_u8)?;
        map.serialize_entry("private-key-42", &2_u8)?;
        map.end()
    }
}

struct MapValueWithoutKey;

impl Serialize for MapValueWithoutKey {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut map = serializer.serialize_map(Some(1))?;
        let _ignored = map.serialize_value(&true);
        map.end()
    }
}

#[test]
fn test_projection_error_accessors_expose_map_key_classification() {
    let mut budget = JsonValueBudget::new(JsonValueLimits::<JsonResource, usize>::new());
    let mut transaction = budget.transaction();
    let error = JsonValueEncoder::new()
        .encode_in_transaction(&UnsupportedSequenceMapKey, &mut transaction)
        .unwrap_err();

    assert!(error.serialization_error().is_some());
    assert_eq!(
        error.kind(),
        Some(crate::encode::JsonSerializationErrorKind::UnsupportedMapKey {
            kind: crate::encode::JsonMapKeyKind::Tuple,
        })
    );
    assert_eq!(
        error.category(),
        Some(crate::encode::JsonSerializationErrorCategory::ObjectKey)
    );
    assert!(error.is_map_key_error());
    assert!(!error.is_number_error());
    assert!(!error.is_raw_value_error());
    assert!(!error.is_serializer_contract_error());
    assert_eq!(error.map_key_kind(), Some(crate::encode::JsonMapKeyKind::Tuple));
    assert_eq!(error.integer_signedness(), None);
    assert_eq!(error.collection_kind(), None);
    assert_eq!(error.serializer_state_error(), None);

    let source = std::error::Error::source(&error).expect("serialization error must be retained as source");
    assert!(source.downcast_ref::<crate::encode::JsonSerializationError>().is_some());
    assert!(error.to_string().contains("unsupported JSON object key"));
}

#[test]
fn test_projection_duplicate_map_key_is_classified_without_retaining_key_text() {
    let mut budget = JsonValueBudget::new(JsonValueLimits::<JsonResource, usize>::new());
    let mut transaction = budget.transaction();
    let error = JsonValueEncoder::new()
        .encode_in_transaction(&DuplicateMapKey, &mut transaction)
        .unwrap_err();

    assert_eq!(
        error.kind(),
        Some(crate::encode::JsonSerializationErrorKind::DuplicateObjectKey)
    );
    assert_eq!(
        error.category(),
        Some(crate::encode::JsonSerializationErrorCategory::ObjectKey)
    );
    assert!(error.is_map_key_error());
    assert_eq!(error.to_string(), "duplicate JSON object key");
    assert!(!error.to_string().contains("private-key-42"));
}

#[test]
fn test_projection_split_map_state_error_is_sticky_and_has_error_source() {
    let mut budget = JsonValueBudget::new(JsonValueLimits::<JsonResource, usize>::new());
    let mut transaction = budget.transaction();
    let error = JsonValueEncoder::new()
        .encode_in_transaction(&MapValueWithoutKey, &mut transaction)
        .unwrap_err();

    assert!(error.is_serializer_contract_error());
    assert_eq!(
        error.serializer_state_error(),
        Some(crate::encode::JsonSerializerStateError::MapValueWithoutKey)
    );
    assert_eq!(
        error.category(),
        Some(crate::encode::JsonSerializationErrorCategory::SerializerContract)
    );
    let source = std::error::Error::source(&error).expect("serializer failure must be retained as source");
    assert!(source.downcast_ref::<crate::encode::JsonSerializationError>().is_some());
    assert_eq!(error.to_string(), "invalid serializer state");
}

struct MixedScalarMapKeys;

impl Serialize for MixedScalarMapKeys {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut map = serializer.serialize_map(Some(4))?;
        map.serialize_entry(&true, "bool")?;
        map.serialize_entry(&-17_i128, "signed")?;
        map.serialize_entry(&u128::MAX, "wide")?;
        map.serialize_entry(&Some('x'), "optional-char")?;
        map.end()
    }
}

#[test]
fn test_projection_serializes_supported_scalar_map_keys_canonically() {
    let projected = JsonValueEncoder::new().encode(&MixedScalarMapKeys).unwrap();
    assert_eq!(
        projected,
        serde_json::json!({
            "true": "bool",
            "-17": "signed",
            "340282366920938463463374607431768211455": "wide",
            "x": "optional-char",
        })
    );
}

struct MapKeySubmittedTwice;

impl Serialize for MapKeySubmittedTwice {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut map = serializer.serialize_map(Some(1))?;
        map.serialize_key("first")?;
        let _ignored = map.serialize_key("second");
        let _ignored = map.end();
        Err(serde::ser::Error::custom("source ignored map state failure"))
    }
}

#[test]
fn test_projection_keeps_first_map_key_state_error_sticky() {
    let error = JsonValueEncoder::new().encode(&MapKeySubmittedTwice).unwrap_err();
    assert_eq!(
        error.serializer_state_error(),
        Some(crate::encode::JsonSerializerStateError::MapKeyAlreadyPending)
    );
}

struct DuplicateStructField;

impl Serialize for DuplicateStructField {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut record = serializer.serialize_struct("DuplicateStructField", 2)?;
        record.serialize_field("same", &1_u8)?;
        let _ignored = record.serialize_field("same", &2_u8);
        record.end()
    }
}

#[test]
fn test_projection_rejects_duplicate_fields_from_custom_struct_serializer() {
    let error = JsonValueEncoder::new().encode(&DuplicateStructField).unwrap_err();
    assert_eq!(
        error.kind(),
        crate::encode::JsonSerializationErrorKind::DuplicateObjectKey
    );
    assert!(error.is_map_key_error());
}

struct CollectedDisplay;

impl std::fmt::Display for CollectedDisplay {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("collected")
    }
}

struct CollectedString;

impl Serialize for CollectedString {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.collect_str(&CollectedDisplay)
    }
}

#[test]
fn test_projection_covers_scalar_serde_events_and_collect_str() {
    let encoder = JsonValueEncoder::new();

    assert_eq!(encoder.encode(&-8_i8).unwrap(), serde_json::json!(-8));
    assert_eq!(encoder.encode(&-16_i16).unwrap(), serde_json::json!(-16));
    assert_eq!(encoder.encode(&17_u16).unwrap(), serde_json::json!(17));
    assert_eq!(encoder.encode(&32_u32).unwrap(), serde_json::json!(32));
    assert_eq!(encoder.encode(&1.25_f32).unwrap(), serde_json::json!(1.25));
    assert_eq!(encoder.encode(&'x').unwrap(), serde_json::json!("x"));
    assert_eq!(encoder.encode(&Option::<u8>::None).unwrap(), serde_json::Value::Null);
    assert_eq!(encoder.encode(&Some(9_u8)).unwrap(), serde_json::json!(9));
    assert_eq!(encoder.encode(&(true, 4_u8)).unwrap(), serde_json::json!([true, 4]));
    assert_eq!(
        encoder.encode(&CollectedString).unwrap(),
        serde_json::json!("collected")
    );
}
