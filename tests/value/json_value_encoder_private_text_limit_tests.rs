// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Public contracts for bounded private-text collection during value projection.

use std::cell::Cell;
use std::collections::BTreeMap;
use std::error::Error as StdError;
use std::fmt;

use qubit_budget::json::JsonResource;
use qubit_budget::json::JsonValueBudget;
use qubit_budget::json::JsonValueLimits;
use qubit_json::value::JsonValueEncodeError;
use qubit_json::value::JsonValueEncoder;
use serde::Serialize;
use serde::Serializer;
use serde::ser::Error as SerializeError;
use serde::ser::SerializeSeq;
use serde::ser::SerializeStruct;
use serde_json::json;
use serde_json::value::RawValue;

const PRIVATE_NUMBER: &str = concat!("$", "serde_json", ":", ":private::Number");

struct Chunks<'a>(&'a Cell<usize>);

impl fmt::Display for Chunks<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for chunk in ["1e", "0000", "0000", "0"] {
            self.0.set(self.0.get() + 1);
            formatter.write_str(chunk)?;
        }
        Ok(())
    }
}

impl Serialize for Chunks<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

struct PrivateNumber<'a>(&'a Cell<usize>);

impl Serialize for PrivateNumber<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut state = serializer.serialize_struct(PRIVATE_NUMBER, 1)?;
        state.serialize_field(PRIVATE_NUMBER, &Chunks(self.0))?;
        state.end()
    }
}

struct SwallowedChunkError(&'static str);

impl fmt::Display for SwallowedChunkError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let _ = formatter.write_str(self.0);
        Ok(())
    }
}

impl Serialize for SwallowedChunkError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

struct SwallowedPrivateNumberError;

impl Serialize for SwallowedPrivateNumberError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut state = serializer.serialize_struct(PRIVATE_NUMBER, 1)?;
        let _ = state.serialize_field(PRIVATE_NUMBER, &SwallowedChunkError("1e000000000"));
        Err(S::Error::custom("outer serialization failure"))
    }
}

struct FailingDisplay;

impl fmt::Display for FailingDisplay {
    fn fmt(&self, _: &mut fmt::Formatter<'_>) -> fmt::Result {
        Err(fmt::Error)
    }
}

impl Serialize for FailingDisplay {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

struct PrivateNumberWith<'a, T>(&'a T);

impl<T: Serialize> Serialize for PrivateNumberWith<'_, T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut state = serializer.serialize_struct(PRIVATE_NUMBER, 1)?;
        state.serialize_field(PRIVATE_NUMBER, self.0)?;
        state.end()
    }
}

#[derive(Serialize)]
struct NestedRaw<'a> {
    value: &'a RawValue,
}

struct BudgetFailureBeforeRaw<'a>(&'a RawValue);

impl Serialize for BudgetFailureBeforeRaw<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut sequence = serializer.serialize_seq(Some(2))?;
        let _ = sequence.serialize_element(&true);
        let _ = sequence.serialize_element(self.0);
        sequence.end()
    }
}

#[test]
fn test_private_number_collect_stops_at_rejected_chunk() {
    let calls = Cell::new(0);
    let mut budget = JsonValueBudget::new(JsonValueLimits::<JsonResource, usize>::new());
    let error = {
        let mut transaction = budget.transaction();
        JsonValueEncoder::new()
            .encode_in_transaction_with_private_text_limit(
                &PrivateNumber(&calls),
                &mut transaction,
                6,
            )
            .unwrap_err()
    };
    assert!(matches!(
        error,
        JsonValueEncodeError::PrivateTextLimitExceeded { maximum: 6 }
    ));
    assert_eq!(calls.get(), 3);
}

#[test]
fn test_private_text_limit_counts_utf8_bytes_and_zero_rejects_raw_null() {
    for (maximum, expected_success) in [(3, false), (4, true)] {
        let raw = RawValue::from_string("\"é\"".to_owned()).unwrap();
        let mut budget = JsonValueBudget::new(JsonValueLimits::<JsonResource, usize>::new());
        let mut transaction = budget.transaction();
        let result = JsonValueEncoder::new().encode_in_transaction_with_private_text_limit(
            &raw,
            &mut transaction,
            maximum,
        );
        assert_eq!(result.is_ok(), expected_success);
        if !expected_success {
            assert_eq!(JsonValueEncoder::new().encode(&raw), Ok(json!("é")));
        }
        if expected_success {
            transaction.commit().unwrap();
        }
    }

    let raw = RawValue::from_string("null".to_owned()).unwrap();
    let mut budget = JsonValueBudget::new(JsonValueLimits::<JsonResource, usize>::new());
    let mut transaction = budget.transaction();
    assert!(matches!(
        JsonValueEncoder::new().encode_in_transaction_with_private_text_limit(
            &raw,
            &mut transaction,
            0
        ),
        Err(JsonValueEncodeError::PrivateTextLimitExceeded { maximum: 0 })
    ));
}

#[test]
fn test_private_text_limit_does_not_apply_to_ordinary_text_or_marker_maps() {
    let mut budget = JsonValueBudget::new(JsonValueLimits::<JsonResource, usize>::new());
    let mut transaction = budget.transaction();
    let ordinary = "ordinary";
    let owner = JsonValueEncoder::new()
        .encode_in_transaction_with_private_text_limit(&ordinary, &mut transaction, 0)
        .unwrap();
    assert_eq!(owner.as_value(), &json!("ordinary"));

    let mut marker_map = BTreeMap::new();
    marker_map.insert(PRIVATE_NUMBER.to_owned(), "text");
    let owner = JsonValueEncoder::new()
        .encode_in_transaction_with_private_text_limit(&marker_map, &mut transaction, 0)
        .unwrap();
    assert_eq!(owner.as_value(), &json!({ PRIVATE_NUMBER: "text" }));
    transaction.commit().unwrap();
}

#[test]
fn test_private_text_limit_is_per_payload_and_does_not_replace_value_budget() {
    let first = RawValue::from_string("null".to_owned()).unwrap();
    let second = RawValue::from_string("null".to_owned()).unwrap();
    let values = [&first, &second];
    let mut budget = JsonValueBudget::new(JsonValueLimits::<JsonResource, usize>::new());
    let mut transaction = budget.transaction();
    JsonValueEncoder::new()
        .encode_in_transaction_with_private_text_limit(&values, &mut transaction, 4)
        .unwrap();
    transaction.commit().unwrap();

    let raw = RawValue::from_string("\"abcdef\"".to_owned()).unwrap();
    let limits = JsonValueLimits::<JsonResource, usize>::builder()
        .max_string_bytes(2)
        .build();
    let mut budget = JsonValueBudget::new(limits);
    let mut transaction = budget.transaction();
    assert!(matches!(
        JsonValueEncoder::new().encode_in_transaction_with_private_text_limit(&raw, &mut transaction, 8),
        Err(JsonValueEncodeError::Budget(error)) if error.resource() == &JsonResource::StringBytes
    ));
}

#[test]
fn test_private_number_keeps_native_number_measurement_and_old_entry_unbounded() {
    let calls = Cell::new(0);
    let limits = JsonValueLimits::<JsonResource, usize>::builder()
        .max_number_bytes(11)
        .build();
    let mut budget = JsonValueBudget::new(limits);
    let mut transaction = budget.transaction();
    let value = JsonValueEncoder::new()
        .encode_in_transaction_with_private_text_limit(&PrivateNumber(&calls), &mut transaction, 11)
        .unwrap();
    assert_eq!(value.as_value(), &json!(1.0));
    assert_eq!(calls.get(), 4);
    transaction.commit().unwrap();

    let calls = Cell::new(0);
    assert_eq!(
        JsonValueEncoder::new()
            .encode(&PrivateNumber(&calls))
            .unwrap(),
        json!(1.0)
    );
    assert_eq!(calls.get(), 4);
}

#[test]
fn test_private_text_limit_stays_sticky_when_display_or_serde_swallows_failure() {
    let mut budget = JsonValueBudget::new(JsonValueLimits::<JsonResource, usize>::new());
    let mut transaction = budget.transaction();
    let error = JsonValueEncoder::new()
        .encode_in_transaction_with_private_text_limit(
            &SwallowedPrivateNumberError,
            &mut transaction,
            6,
        )
        .unwrap_err();
    assert!(matches!(
        error,
        JsonValueEncodeError::PrivateTextLimitExceeded { maximum: 6 }
    ));
    assert!(error.source().is_none());
    assert_eq!(error.serialization_error(), None);
    assert_eq!(error.kind(), None);
    assert_eq!(error.category(), None);
    assert!(!error.is_number_error());
    assert!(!error.is_map_key_error());
    assert!(!error.is_raw_value_error());
    assert!(!error.is_serializer_contract_error());
    assert_eq!(error.integer_signedness(), None);
    assert_eq!(error.map_key_kind(), None);
    assert_eq!(error.collection_kind(), None);
    assert_eq!(error.serializer_state_error(), None);
    assert!(!error.to_string().contains("1e000000000"));
    assert!(!format!("{error:?}").contains("1e000000000"));
}

#[test]
fn test_private_display_formatting_error_keeps_existing_classification() {
    let mut budget = JsonValueBudget::new(JsonValueLimits::<JsonResource, usize>::new());
    let mut transaction = budget.transaction();
    let error = JsonValueEncoder::new()
        .encode_in_transaction_with_private_text_limit(
            &PrivateNumberWith(&FailingDisplay),
            &mut transaction,
            100,
        )
        .unwrap_err();
    assert!(matches!(
        error,
        JsonValueEncodeError::Serialization(error)
            if error.kind() == qubit_json::encode::JsonSerializationErrorKind::DisplayFormattingFailed
    ));
}

#[test]
fn test_private_text_limit_failure_does_not_replace_first_budget_failure() {
    let raw = RawValue::from_string("null".to_owned()).unwrap();
    let limits = JsonValueLimits::<JsonResource, usize>::builder()
        .max_nodes(1)
        .build();
    let mut budget = JsonValueBudget::new(limits);
    let mut transaction = budget.transaction();
    assert!(matches!(
        JsonValueEncoder::new().encode_in_transaction_with_private_text_limit(
            &BudgetFailureBeforeRaw(&raw),
            &mut transaction,
            0
        ),
        Err(JsonValueEncodeError::Budget(_))
    ));
}

#[test]
fn test_private_text_limit_failure_can_be_dropped_before_next_transaction() {
    let raw = RawValue::from_string("\"too long\"".to_owned()).unwrap();
    let limits = JsonValueLimits::<JsonResource, usize>::builder()
        .max_nodes(3)
        .max_payload_bytes(16)
        .build();
    let mut budget = JsonValueBudget::new(limits);
    {
        let mut transaction = budget.transaction();
        JsonValueEncoder::new()
            .encode_in_transaction_with_private_text_limit(&true, &mut transaction, 1)
            .unwrap();
        assert!(matches!(
            JsonValueEncoder::new().encode_in_transaction_with_private_text_limit(
                &raw,
                &mut transaction,
                1
            ),
            Err(JsonValueEncodeError::PrivateTextLimitExceeded { maximum: 1 })
        ));
    }
    let mut transaction = budget.transaction();
    JsonValueEncoder::new()
        .encode_in_transaction_with_private_text_limit(&[true, true], &mut transaction, 1)
        .unwrap();
    transaction.commit().unwrap();
    assert_eq!(budget.used_nodes(), Some(3));
}

#[test]
fn test_private_text_limit_reaches_nested_object_payloads_and_u64_budget() {
    let raw = RawValue::from_string("null".to_owned()).unwrap();
    let nested = NestedRaw { value: &raw };
    let mut budget = JsonValueBudget::new(JsonValueLimits::<JsonResource, u64>::new());
    let mut transaction = budget.transaction();
    let error = JsonValueEncoder::new()
        .encode_in_transaction_with_private_text_limit(&nested, &mut transaction, 3)
        .unwrap_err();
    assert!(matches!(
        error,
        JsonValueEncodeError::PrivateTextLimitExceeded { maximum: 3 }
    ));

    let raw = RawValue::from_string("[0]".to_owned()).unwrap();
    let mut budget = JsonValueBudget::new(JsonValueLimits::<JsonResource, u64>::new());
    let mut transaction = budget.transaction();
    JsonValueEncoder::new()
        .encode_in_transaction_with_private_text_limit(&raw, &mut transaction, 3)
        .unwrap();
    transaction.commit().unwrap();
}

#[test]
fn test_private_text_limit_includes_exact_raw_boundary() {
    let raw = RawValue::from_string("[0]".to_owned()).unwrap();
    let mut budget = JsonValueBudget::new(JsonValueLimits::<JsonResource, usize>::new());
    let mut transaction = budget.transaction();
    let owner = JsonValueEncoder::new()
        .encode_in_transaction_with_private_text_limit(&raw, &mut transaction, 3)
        .unwrap();
    assert_eq!(owner.as_value(), &json!([0]));
    transaction.commit().unwrap();
}

#[test]
fn test_private_text_limit_rejects_before_raw_materialization() {
    let raw = RawValue::from_string("[0]".to_owned()).unwrap();
    let mut budget = JsonValueBudget::new(JsonValueLimits::<JsonResource, usize>::new());
    let mut transaction = budget.transaction();
    let error = JsonValueEncoder::new()
        .encode_in_transaction_with_private_text_limit(&raw, &mut transaction, 2)
        .unwrap_err();
    assert!(matches!(
        error,
        JsonValueEncodeError::PrivateTextLimitExceeded { maximum: 2 }
    ));
}
