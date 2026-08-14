// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Behavior tests for budget-aware JSON serialization.
// qubit-style: allow explicit-imports

use std::cell::Cell;
use std::collections::BTreeMap;
use std::fmt;

use qubit_budget::BudgetError;
use qubit_budget::Observation;
use qubit_budget::json::JsonResource;
use qubit_json::text::JsonEncodeError;
use qubit_json::text::encode_to_vec;
use serde::Serialize;
use serde::Serializer;
use serde::ser::SerializeMap;
use serde::ser::SerializeSeq;
use serde::ser::SerializeStruct;
use serde::ser::SerializeStructVariant;
use serde::ser::SerializeTupleStruct;
use serde::ser::SerializeTupleVariant;
use serde_json::Number;
use serde_json::to_vec;
use serde_json::value::RawValue;

use super::super::json_test_limits_tests::JsonTestLimits;

/// Private token used by serde_json for arbitrary-precision numbers.
const JSON_NUMBER_TOKEN: &str =
    concat!("$", "serde_json", ":", ":private::Number");

/// Private token used by serde_json for raw JSON fragments.
const JSON_RAW_VALUE_TOKEN: &str =
    concat!("$", "serde_json", ":", ":private::RawValue");

struct SkipFieldStruct;

impl Serialize for SkipFieldStruct {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("SkipFieldStruct", 1)?;
        state.skip_field("omitted")?;
        state.serialize_field("kept", &true)?;
        state.end()
    }
}

/// Verifies skipped struct fields pass through the compound wrapper without
/// affecting accounting or output.
#[test]
fn test_json_encode_compound_forwards_skip_field() {
    let mut session = JsonTestLimits::new().encode_session();
    let encoded = encode_to_vec(&SkipFieldStruct, &mut session)
        .expect("skipped fields should be forwarded");
    assert_eq!(encoded, br#"{"kept":true}"#);
}

struct SkipFieldVariant;

impl Serialize for SkipFieldVariant {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct_variant("V", 0, "V", 1)?;
        state.skip_field("omitted")?;
        state.end()
    }
}

/// Verifies struct-variant skip fields are forwarded through the compound.
#[test]
fn test_json_encode_compound_forwards_struct_variant_skip_field() {
    let mut session = JsonTestLimits::new().encode_session();
    let output = encode_to_vec(&SkipFieldVariant, &mut session)
        .expect("skipped struct-variant fields should encode");
    assert_eq!(output, br#"{"V":{}}"#);
}

/// Number text that serde_json deserializes through its private map token.
const LARGE_NUMBER_TEXT: &str = "123456789012345678901234567890";

/// Display value that exposes how many chunks formatting reached.
struct CountedDisplay<'a> {
    /// Number of chunks requested by the formatter.
    formatted: &'a Cell<usize>,

    /// Text emitted for every chunk.
    chunk: &'static str,

    /// Total number of available chunks.
    chunks: usize,
}

impl fmt::Display for CountedDisplay<'_> {
    /// Emits fixed chunks while recording each reached formatting step.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for _ in 0..self.chunks {
            self.formatted.set(self.formatted.get() + 1);
            formatter.write_str(self.chunk)?;
        }
        Ok(())
    }
}

/// Value serialized exclusively through `Serializer::collect_str`.
struct CollectedText<'a>(CountedDisplay<'a>);

impl Serialize for CollectedText<'_> {
    /// Delegates the display value to Serde's collection hook.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_str(&self.0)
    }
}

/// Single-entry map whose key is emitted through `collect_str`.
struct CollectedKeyMap<'a>(CollectedText<'a>);

impl Serialize for CollectedKeyMap<'_> {
    /// Serializes one collected key and a null value.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(1))?;
        map.serialize_entry(&self.0, &())?;
        map.end()
    }
}

/// Private serde_json text wrapper whose payload uses `collect_str`.
struct CollectedPrivateText<'a> {
    /// Private struct token to emit.
    token: &'static str,

    /// Display payload observed by the budget decorator.
    payload: CountedDisplay<'a>,
}

/// Scalar that records when its own serializer is entered.
struct ObservedUnit<'a>(&'a Cell<usize>);

impl Serialize for ObservedUnit<'_> {
    /// Records traversal and emits JSON null.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.0.set(self.0.get() + 1);
        serializer.serialize_unit()
    }
}

/// Sequence that declares one item but emits three.
struct UnderreportedSequence<'a>(&'a Cell<usize>);

impl Serialize for UnderreportedSequence<'_> {
    /// Emits three values through a one-item sequence declaration.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut sequence = serializer.serialize_seq(Some(1))?;
        for _ in 0..3 {
            sequence.serialize_element(&ObservedUnit(self.0))?;
        }
        sequence.end()
    }
}

/// Sequence that declares three items but emits one.
struct OverreportedSequence;

impl Serialize for OverreportedSequence {
    /// Emits one value through a three-item sequence declaration.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut sequence = serializer.serialize_seq(Some(3))?;
        sequence.serialize_element(&())?;
        sequence.end()
    }
}

/// Map that declares one entry but emits three.
struct UnderreportedMap<'a>(&'a Cell<usize>);

impl Serialize for UnderreportedMap<'_> {
    /// Emits three entries through a one-entry map declaration.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(1))?;
        for key in ["a", "b", "c"] {
            map.serialize_entry(key, &ObservedUnit(self.0))?;
        }
        map.end()
    }
}

/// Struct that declares one field but emits three.
struct UnderreportedStruct<'a>(&'a Cell<usize>);

impl Serialize for UnderreportedStruct<'_> {
    /// Emits three fields through a one-field struct declaration.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state =
            serializer.serialize_struct("UnderreportedStruct", 1)?;
        state.serialize_field("a", &ObservedUnit(self.0))?;
        state.serialize_field("b", &ObservedUnit(self.0))?;
        state.serialize_field("c", &ObservedUnit(self.0))?;
        state.end()
    }
}

/// Tuple struct that declares one field but emits three.
struct UnderreportedTupleStruct<'a>(&'a Cell<usize>);

impl Serialize for UnderreportedTupleStruct<'_> {
    /// Emits three fields through a one-field tuple-struct declaration.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state =
            serializer.serialize_tuple_struct("Underreported", 1)?;
        for _ in 0..3 {
            state.serialize_field(&ObservedUnit(self.0))?;
        }
        state.end()
    }
}

/// Enum variants that intentionally underreport their field count.
struct UnderreportedVariant<'a> {
    /// Child traversal counter.
    observed: &'a Cell<usize>,

    /// Selects tuple or struct variant encoding.
    tuple: bool,
}

impl Serialize for UnderreportedVariant<'_> {
    /// Emits three fields through a one-field variant declaration.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if self.tuple {
            let mut state =
                serializer.serialize_tuple_variant("E", 0, "V", 1)?;
            for _ in 0..3 {
                state.serialize_field(&ObservedUnit(self.observed))?;
            }
            state.end()
        } else {
            let mut state =
                serializer.serialize_struct_variant("E", 0, "V", 1)?;
            state.serialize_field("a", &ObservedUnit(self.observed))?;
            state.serialize_field("b", &ObservedUnit(self.observed))?;
            state.serialize_field("c", &ObservedUnit(self.observed))?;
            state.end()
        }
    }
}

impl Serialize for CollectedPrivateText<'_> {
    /// Emits one private field containing a collected display payload.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct(self.token, 1)?;
        state.serialize_field(
            self.token,
            &CollectedText(CountedDisplay {
                formatted: self.payload.formatted,
                chunk: self.payload.chunk,
                chunks: self.payload.chunks,
            }),
        )?;
        state.end()
    }
}

/// Asserts that serialization failed for the expected JSON resource.
fn assert_resource<T>(value: &T, limits: JsonTestLimits, expected: JsonResource)
where
    T: Serialize + ?Sized,
{
    let mut budget = limits.encode_session();
    let error = encode_to_vec(value, &mut budget)
        .expect_err("the configured JSON limit must reject the value");
    let JsonEncodeError::Budget(error) = error else {
        panic!("expected a budget error, got {error:?}");
    };
    assert_eq!(
        error
            .budget_error()
            .expect("the error must contain a budget failure")
            .resource(),
        &expected,
    );
}

/// Asserts that the budget-aware adapter preserves serde_json output.
fn assert_same_json<T>(value: &T)
where
    T: Serialize + ?Sized,
{
    let expected = to_vec(value).expect("reference JSON");
    let mut budget = JsonTestLimits::new().encode_session();
    let actual = encode_to_vec(value, &mut budget)
        .expect("budget-aware JSON should serialize");
    assert_eq!(actual, expected);
}

/// Asserts the exact JSON node count assigned to one serializable value.
fn assert_node_count<T>(value: &T, nodes: usize)
where
    T: Serialize + ?Sized,
{
    let mut exact =
        JsonTestLimits::new().with_max_nodes(nodes).encode_session();
    encode_to_vec(value, &mut exact)
        .expect("the exact node budget should accept the value");

    let mut insufficient = JsonTestLimits::new()
        .with_max_nodes(nodes.saturating_sub(1))
        .encode_session();
    let error = encode_to_vec(value, &mut insufficient)
        .expect_err("one fewer node must reject the value");
    let JsonEncodeError::Budget(error) = error else {
        panic!("expected a budget error, got {error:?}");
    };
    assert_eq!(
        error
            .budget_error()
            .expect("the error must contain a budget failure")
            .resource(),
        &JsonResource::Nodes,
    );
}

struct UnknownSequence(usize);

impl Serialize for UnknownSequence {
    /// Serializes a sequence without declaring its length.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut sequence = serializer.serialize_seq(None)?;
        for value in 0..self.0 {
            sequence.serialize_element(&value)?;
        }
        sequence.end()
    }
}

struct UnknownMap(usize);

impl Serialize for UnknownMap {
    /// Serializes a map without declaring its length.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(None)?;
        for value in 0..self.0 {
            map.serialize_entry(&value, &value)?;
        }
        map.end()
    }
}

struct CountedUnknownSequence<'a> {
    serialized: &'a Cell<usize>,
    len: usize,
}

/// Unknown-length map that counts entries reached by serialization.
struct CountedUnknownMap<'a> {
    /// Number of entries entered before the serializer stopped.
    serialized: &'a Cell<usize>,

    /// Total entries offered to the serializer.
    len: usize,
}

/// Simulates serde_json's private number shape with an observable payload.
struct CountedPrivateNumber<'a> {
    /// Number of times the private string payload was entered.
    serialized: &'a Cell<usize>,
}

impl Serialize for CountedPrivateNumber<'_> {
    /// Emits the private number token and one observable string payload.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct(JSON_NUMBER_TOKEN, 1)?;
        state.serialize_field(
            JSON_NUMBER_TOKEN,
            &CountedPrivateNumberPayload {
                serialized: self.serialized,
            },
        )?;
        state.end()
    }
}

/// Observable private number string payload.
struct CountedPrivateNumberPayload<'a> {
    /// Number of times this payload was asked to serialize.
    serialized: &'a Cell<usize>,
}

impl Serialize for CountedPrivateNumberPayload<'_> {
    /// Records entry before emitting a valid arbitrary-precision number text.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.serialized.set(self.serialized.get() + 1);
        serializer.serialize_str("1")
    }
}

/// Simulates serde_json's private raw-value shape with an observable payload.
struct CountedPrivateRawValue<'a> {
    /// Raw JSON fragment emitted by the private string payload.
    raw: &'a str,

    /// Number of times the private string payload was entered.
    serialized: &'a Cell<usize>,
}

impl Serialize for CountedPrivateRawValue<'_> {
    /// Emits the private raw-value token and one observable string payload.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct(JSON_RAW_VALUE_TOKEN, 1)?;
        state.serialize_field(
            JSON_RAW_VALUE_TOKEN,
            &CountedPrivateRawValuePayload {
                raw: self.raw,
                serialized: self.serialized,
            },
        )?;
        state.end()
    }
}

/// Observable private raw-value string payload.
struct CountedPrivateRawValuePayload<'a> {
    /// Raw JSON fragment passed to serde_json's emitter.
    raw: &'a str,

    /// Number of times this payload was asked to serialize.
    serialized: &'a Cell<usize>,
}

impl Serialize for CountedPrivateRawValuePayload<'_> {
    /// Records entry before emitting the raw JSON fragment as a private string.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.serialized.set(self.serialized.get() + 1);
        serializer.serialize_str(self.raw)
    }
}

/// Serializes one raw value before an observable trailing sequence element.
struct RawThenCountedTail<'a> {
    /// Raw JSON value serialized as the first sequence element.
    raw: &'a RawValue,

    /// Number of times traversal reached the trailing element.
    serialized_tail: &'a Cell<usize>,
}

impl Serialize for RawThenCountedTail<'_> {
    /// Emits the raw value, then records whether traversal continued past it.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut sequence = serializer.serialize_seq(Some(2))?;
        sequence.serialize_element(self.raw)?;
        self.serialized_tail.set(self.serialized_tail.get() + 1);
        sequence.serialize_element(&())?;
        sequence.end()
    }
}

impl Serialize for CountedUnknownSequence<'_> {
    /// Serializes a counted sequence without declaring its length.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut sequence = serializer.serialize_seq(None)?;
        for value in 0..self.len {
            self.serialized.set(self.serialized.get() + 1);
            sequence.serialize_element(&value)?;
        }
        sequence.end()
    }
}

impl Serialize for CountedUnknownMap<'_> {
    /// Serializes a counted map without declaring its length.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(None)?;
        for value in 0..self.len {
            self.serialized.set(self.serialized.get() + 1);
            map.serialize_entry(&value, &value)?;
        }
        map.end()
    }
}

#[derive(Serialize)]
struct Nested {
    value: Vec<Vec<bool>>,
}

/// Struct whose keys and scalar payloads share the cumulative payload budget.
#[derive(Serialize)]
struct SharedPayload {
    /// Two-byte JSON string payload under a four-byte key.
    text: &'static str,
    /// Two-byte JSON number payload under a six-byte key.
    number: u8,
}

#[derive(Serialize)]
struct Newtype(u8);

#[derive(Serialize)]
enum EnumShape {
    Unit,
    Newtype(u8),
    Tuple(u8, u8),
    Struct { value: u8 },
}

/// Verifies scalar values consume one JSON node.
#[test]
fn test_json_encode_serializer_charges_scalar_nodes() {
    assert_resource(
        &true,
        JsonTestLimits::new().with_max_nodes(0),
        JsonResource::Nodes,
    );
}

/// Verifies known and unknown sequences enforce their actual item count.
#[test]
fn test_json_encode_serializer_checks_sequence_items() {
    let limits = JsonTestLimits::new().with_max_sequence_items(2);
    let mut exact = limits.clone().encode_session();
    encode_to_vec(&UnknownSequence(2), &mut exact)
        .expect("an unknown sequence at the exact item limit must serialize");
    assert_resource(&[1, 2, 3], limits, JsonResource::SequenceItems);
    assert_resource(
        &UnknownSequence(3),
        JsonTestLimits::new().with_max_sequence_items(2),
        JsonResource::SequenceItems,
    );
}

/// Verifies an unknown-length sequence is admitted after its actual item count
/// is observed.
#[test]
fn test_json_encode_serializer_checks_unknown_sequence_after_traversal() {
    let serialized = Cell::new(0);
    let value = CountedUnknownSequence {
        serialized: &serialized,
        len: 1_000,
    };

    assert_resource(
        &value,
        JsonTestLimits::new().with_max_sequence_items(2),
        JsonResource::SequenceItems,
    );
    assert_eq!(serialized.get(), value.len);
}

/// Verifies known and unknown maps enforce their actual entry count.
#[test]
fn test_json_encode_serializer_checks_map_entries() {
    let limits = JsonTestLimits::new().with_max_map_entries(1);
    let values = BTreeMap::from([("a", 1), ("b", 2)]);
    assert_resource(&values, limits, JsonResource::MapEntries);
    let mut exact = JsonTestLimits::new()
        .with_max_map_entries(2)
        .encode_session();
    encode_to_vec(&UnknownMap(2), &mut exact)
        .expect("an unknown map at the exact entry limit must serialize");
    assert_resource(
        &UnknownMap(2),
        JsonTestLimits::new().with_max_map_entries(1),
        JsonResource::MapEntries,
    );
}

/// Verifies an unknown-length map is admitted after its actual entry count is
/// observed.
#[test]
fn test_json_encode_serializer_checks_unknown_map_after_traversal() {
    let serialized = Cell::new(0);
    let value = CountedUnknownMap {
        serialized: &serialized,
        len: 1_000,
    };

    assert_resource(
        &value,
        JsonTestLimits::new().with_max_map_entries(2),
        JsonResource::MapEntries,
    );
    assert_eq!(serialized.get(), value.len);
}

/// Verifies nested output is checked with root-inclusive JSON depth.
#[test]
fn test_json_encode_serializer_checks_nested_depth() {
    let value = Nested {
        value: vec![vec![true]],
    };
    assert_resource(
        &value,
        JsonTestLimits::new().with_max_depth(3),
        JsonResource::Depth,
    );
}

/// Verifies keys and strings are measured in UTF-8 bytes.
#[test]
fn test_json_encode_serializer_checks_utf8_key_and_string_bytes() {
    let values = BTreeMap::from([("你好", true)]);
    assert_resource(
        &values,
        JsonTestLimits::new().with_max_key_bytes(5),
        JsonResource::KeyBytes,
    );
    assert_resource(
        &"你好",
        JsonTestLimits::new().with_max_string_bytes(5),
        JsonResource::StringBytes,
    );
}

/// Verifies keys, strings, and numbers consume one shared encode payload.
#[test]
fn test_json_encode_serializer_consumes_shared_payload() {
    assert_resource(
        &SharedPayload {
            text: "bc",
            number: 12,
        },
        JsonTestLimits::new().with_max_payload_bytes(13),
        JsonResource::PayloadBytes,
    );
}

/// Verifies integer and finite-float JSON text lengths are checked.
#[test]
fn test_json_encode_serializer_checks_number_bytes() {
    assert_resource(
        &-123_i32,
        JsonTestLimits::new().with_max_number_bytes(3),
        JsonResource::NumberBytes,
    );
    assert_resource(
        &12.25_f64,
        JsonTestLimits::new().with_max_number_bytes(4),
        JsonResource::NumberBytes,
    );
    assert_resource(
        &1.0_f64,
        JsonTestLimits::new().with_max_number_bytes(2),
        JsonResource::NumberBytes,
    );
}

/// Verifies serde_json's private number token is counted as one number node.
#[test]
fn test_json_encode_serializer_charges_arbitrary_precision_number_once() {
    let number = LARGE_NUMBER_TEXT
        .parse::<Number>()
        .expect("the arbitrary-precision number should parse");
    assert_resource(
        &number,
        JsonTestLimits::new()
            .with_max_number_bytes(LARGE_NUMBER_TEXT.len() - 1),
        JsonResource::NumberBytes,
    );
    assert_node_count(&number, 1);
}

/// Verifies private Number map metadata does not consume object limits.
#[test]
fn test_json_preflight_ignores_private_number_map_metadata() {
    let number = LARGE_NUMBER_TEXT
        .parse::<Number>()
        .expect("the arbitrary-precision number should parse");
    let mut budget = JsonTestLimits::new()
        .with_max_nodes(1)
        .with_max_map_entries(0)
        .with_max_key_bytes(0)
        .with_max_number_bytes(LARGE_NUMBER_TEXT.len())
        .encode_session();

    let output = encode_to_vec(&number, &mut budget)
        .expect("private Number metadata must not consume JSON object limits");

    assert_eq!(output, LARGE_NUMBER_TEXT.as_bytes());
}

/// Verifies private Number metadata inside RawValue is not charged as a map.
#[test]
fn test_json_preflight_ignores_private_number_map_metadata_inside_raw_value() {
    let raw = RawValue::from_string(String::from(LARGE_NUMBER_TEXT))
        .expect("fixture must contain valid raw JSON");
    let mut budget = JsonTestLimits::new()
        .with_max_nodes(1)
        .with_max_map_entries(0)
        .with_max_key_bytes(0)
        .with_max_string_bytes(0)
        .with_max_number_bytes(LARGE_NUMBER_TEXT.len())
        .encode_session();

    let output = encode_to_vec(&raw, &mut budget)
        .expect("raw private Number metadata must not consume object limits");

    assert_eq!(output, LARGE_NUMBER_TEXT.as_bytes());
    assert_resource(
        &raw,
        JsonTestLimits::new()
            .with_max_number_bytes(LARGE_NUMBER_TEXT.len() - 1),
        JsonResource::NumberBytes,
    );
}

/// Verifies RawValue object text cannot impersonate serde_json's Number token.
#[test]
fn test_raw_value_charges_single_number_token_object_as_object() {
    let raw =
        RawValue::from_string(format!(r#"{{"{JSON_NUMBER_TOKEN}":"x"}}"#))
            .expect("fixture must contain valid raw JSON");
    let mut budget =
        JsonTestLimits::new().with_max_key_bytes(0).encode_session();

    let error = encode_to_vec(&raw, &mut budget)
        .expect_err("the raw object's key must consume the key budget");

    assert!(matches!(
        error,
        JsonEncodeError::Budget(error)
            if matches!(
                error.budget_error(),
                Some(BudgetError::LimitExceeded {
                    resource: JsonResource::KeyBytes,
                    observed: Observation::Exact(actual),
                    maximum: 0,
                }) if *actual == JSON_NUMBER_TOKEN.len()
            )
    ));
}

/// Verifies collected string text is rejected after its complete byte length is
/// measured.
#[test]
fn test_collect_str_rejects_after_complete_formatting() {
    let formatted = Cell::new(0);
    let value = CollectedText(CountedDisplay {
        formatted: &formatted,
        chunk: "ab",
        chunks: 1_000,
    });

    assert_resource(
        &value,
        JsonTestLimits::new().with_max_string_bytes(3),
        JsonResource::StringBytes,
    );
    assert_eq!(formatted.get(), 1_000);
}

/// Verifies collected map keys are rejected after their complete byte length
/// is measured.
#[test]
fn test_collect_str_map_key_rejects_after_complete_formatting() {
    let formatted = Cell::new(0);
    let map = CollectedKeyMap(CollectedText(CountedDisplay {
        formatted: &formatted,
        chunk: "ab",
        chunks: 1_000,
    }));

    assert_resource(
        &map,
        JsonTestLimits::new().with_max_key_bytes(3),
        JsonResource::KeyBytes,
    );
    assert_eq!(formatted.get(), 1_000);
}

/// Verifies private Number collect_str payloads retain number budgeting after
/// complete formatting.
#[test]
fn test_private_number_collect_str_rejects_after_complete_formatting() {
    let formatted = Cell::new(0);
    let value = CollectedPrivateText {
        token: JSON_NUMBER_TOKEN,
        payload: CountedDisplay {
            formatted: &formatted,
            chunk: "12",
            chunks: 1_000,
        },
    };

    assert_resource(
        &value,
        JsonTestLimits::new().with_max_number_bytes(3),
        JsonResource::NumberBytes,
    );
    assert_eq!(formatted.get(), 1_000);
}

/// Verifies private RawValue collect_str payloads retain output budgeting.
#[test]
fn test_private_raw_value_collect_str_rejects_during_formatting() {
    let formatted = Cell::new(0);
    let value = CollectedPrivateText {
        token: JSON_RAW_VALUE_TOKEN,
        payload: CountedDisplay {
            formatted: &formatted,
            chunk: "null ",
            chunks: 1_000,
        },
    };

    assert_resource(
        &value,
        JsonTestLimits::new().with_max_output_bytes(8),
        JsonResource::OutputBytes,
    );
    assert!(formatted.get() < 1_000);
}

/// Verifies output-only limits bound ordinary collect_str allocation.
#[test]
fn test_output_only_limit_stops_collect_str_formatting() {
    let formatted = Cell::new(0);
    let value = CollectedText(CountedDisplay {
        formatted: &formatted,
        chunk: "ab",
        chunks: 1_000,
    });

    assert_resource(
        &value,
        JsonTestLimits::new().with_max_output_bytes(3),
        JsonResource::OutputBytes,
    );
    assert!(formatted.get() < 1_000);
}

/// Verifies output-only limits bound collected map-key allocation.
#[test]
fn test_output_only_limit_stops_map_key_collect_str_formatting() {
    let formatted = Cell::new(0);
    let map = CollectedKeyMap(CollectedText(CountedDisplay {
        formatted: &formatted,
        chunk: "ab",
        chunks: 1_000,
    }));

    assert_resource(
        &map,
        JsonTestLimits::new().with_max_output_bytes(3),
        JsonResource::OutputBytes,
    );
    assert!(formatted.get() < 1_000);
}

/// Verifies output-only limits bound private Number collect_str allocation.
#[test]
fn test_output_only_limit_stops_private_number_collect_str_formatting() {
    let formatted = Cell::new(0);
    let value = CollectedPrivateText {
        token: JSON_NUMBER_TOKEN,
        payload: CountedDisplay {
            formatted: &formatted,
            chunk: "12",
            chunks: 1_000,
        },
    };

    assert_resource(
        &value,
        JsonTestLimits::new().with_max_output_bytes(3),
        JsonResource::OutputBytes,
    );
    assert!(formatted.get() < 1_000);
}

/// Verifies conservative collect_str output checks preserve exact boundaries.
#[test]
fn test_collect_str_output_lower_bound_preserves_valid_boundaries() {
    let formatted = Cell::new(0);
    let value = CollectedText(CountedDisplay {
        formatted: &formatted,
        chunk: "a",
        chunks: 1,
    });
    let mut budget = JsonTestLimits::new()
        .with_max_output_bytes(3)
        .encode_session();
    assert_eq!(
        encode_to_vec(&value, &mut budget)
            .expect("quoted collected string must fit its exact output limit"),
        br#""a""#,
    );

    let formatted = Cell::new(0);
    let map = CollectedKeyMap(CollectedText(CountedDisplay {
        formatted: &formatted,
        chunk: "a",
        chunks: 1,
    }));
    let mut budget = JsonTestLimits::new()
        .with_max_output_bytes(10)
        .encode_session();
    assert_eq!(
        encode_to_vec(&map, &mut budget)
            .expect("collected map key must fit its exact output limit"),
        br#"{"a":null}"#,
    );

    let formatted = Cell::new(0);
    let number = CollectedPrivateText {
        token: JSON_NUMBER_TOKEN,
        payload: CountedDisplay {
            formatted: &formatted,
            chunk: "123",
            chunks: 1,
        },
    };
    let mut budget = JsonTestLimits::new()
        .with_max_output_bytes(3)
        .encode_session();
    assert_eq!(
        encode_to_vec(&number, &mut budget)
            .expect("collected Number must fit its exact output limit"),
        b"123",
    );
}

/// Verifies a low sequence length hint is rejected after the actual item count
/// is observed at compound completion.
#[test]
fn test_underreported_sequence_is_rejected_after_third_child() {
    let observed = Cell::new(0);
    assert_resource(
        &UnderreportedSequence(&observed),
        JsonTestLimits::new().with_max_sequence_items(2),
        JsonResource::SequenceItems,
    );
    assert_eq!(observed.get(), 3);
}

/// Verifies a high length hint does not charge items that were never emitted.
#[test]
fn test_overreported_sequence_charges_actual_items_only() {
    let mut session = JsonTestLimits::new()
        .with_max_sequence_items(1)
        .encode_session();

    let output = encode_to_vec(&OverreportedSequence, &mut session)
        .expect("only the emitted sequence item must be charged");

    assert_eq!(output, b"[null]");
}

/// Verifies a low map length hint is rejected after the actual entry count is
/// observed at compound completion.
#[test]
fn test_underreported_map_is_rejected_after_third_value() {
    let observed = Cell::new(0);
    assert_resource(
        &UnderreportedMap(&observed),
        JsonTestLimits::new().with_max_map_entries(2),
        JsonResource::MapEntries,
    );
    assert_eq!(observed.get(), 3);
}

/// Verifies a low struct length is rejected after the actual field count is
/// observed at compound completion.
#[test]
fn test_underreported_struct_is_rejected_after_third_value() {
    let observed = Cell::new(0);
    assert_resource(
        &UnderreportedStruct(&observed),
        JsonTestLimits::new().with_max_map_entries(2),
        JsonResource::MapEntries,
    );
    assert_eq!(observed.get(), 3);
}

/// Verifies a low tuple-struct length is rejected after the actual item count
/// is observed at compound completion.
#[test]
fn test_underreported_tuple_struct_is_rejected_after_third_value() {
    let observed = Cell::new(0);
    assert_resource(
        &UnderreportedTupleStruct(&observed),
        JsonTestLimits::new().with_max_sequence_items(2),
        JsonResource::SequenceItems,
    );
    assert_eq!(observed.get(), 3);
}

/// Verifies low tuple-variant lengths are rejected after the actual item count
/// is observed at compound completion.
#[test]
fn test_underreported_tuple_variant_is_rejected_after_third_value() {
    let observed = Cell::new(0);
    assert_resource(
        &UnderreportedVariant {
            observed: &observed,
            tuple: true,
        },
        JsonTestLimits::new().with_max_sequence_items(2),
        JsonResource::SequenceItems,
    );
    assert_eq!(observed.get(), 3);
}

/// Verifies low struct-variant lengths are rejected after the actual entry
/// count is observed at compound completion.
#[test]
fn test_underreported_struct_variant_is_rejected_after_third_value() {
    let observed = Cell::new(0);
    assert_resource(
        &UnderreportedVariant {
            observed: &observed,
            tuple: false,
        },
        JsonTestLimits::new().with_max_map_entries(2),
        JsonResource::MapEntries,
    );
    assert_eq!(observed.get(), 3);
}

/// Verifies private number node and depth limits reject after their payload is
/// collected into one complete measurement.
#[test]
fn test_json_encode_serializer_checks_private_number_after_payload_collection()
{
    let serialized = Cell::new(0);
    let value = CountedPrivateNumber {
        serialized: &serialized,
    };

    assert_resource(
        &value,
        JsonTestLimits::new().with_max_nodes(0),
        JsonResource::Nodes,
    );
    assert_eq!(serialized.get(), 1);

    serialized.set(0);

    assert_resource(
        &value,
        JsonTestLimits::new().with_max_depth(0),
        JsonResource::Depth,
    );
    assert_eq!(serialized.get(), 1);
}

/// Verifies simulated raw values ignore their private key and string shape.
#[test]
fn test_json_encode_serializer_measures_simulated_raw_value_structure_once() {
    let serialized = Cell::new(0);
    let value = CountedPrivateRawValue {
        raw: "123",
        serialized: &serialized,
    };
    let mut budget = JsonTestLimits::new()
        .with_max_nodes(1)
        .with_max_key_bytes(0)
        .with_max_string_bytes(0)
        .with_max_number_bytes(3)
        .encode_session();

    let output = encode_to_vec(&value, &mut budget).expect(
        "private raw metadata must not consume JSON key or string limits",
    );

    assert_eq!(output, b"123");
    assert_eq!(serialized.get(), 1);
}

/// Verifies actual RawValue output is measured as its represented JSON value.
#[test]
fn test_json_encode_serializer_measures_actual_raw_value_structure() {
    let raw = RawValue::from_string(String::from(r#"{"ok":[1,true]}"#))
        .expect("fixture must contain valid raw JSON");
    let mut budget = JsonTestLimits::new()
        .with_max_nodes(4)
        .with_max_map_entries(1)
        .with_max_sequence_items(2)
        .with_max_key_bytes(2)
        .with_max_string_bytes(0)
        .encode_session();

    let output = encode_to_vec(&raw, &mut budget)
        .expect("the raw JSON structure should fit its exact limits");

    assert_eq!(output, br#"{"ok":[1,true]}"#);
}

/// Verifies raw node failures occur after the raw fragment and its following
/// value have been traversed into the complete measurement.
#[test]
fn test_json_encode_serializer_checks_raw_nodes_after_tail_traversal() {
    let raw = RawValue::from_string(String::from("[null,null]"))
        .expect("fixture must contain valid raw JSON");
    let serialized_tail = Cell::new(0);
    let value = RawThenCountedTail {
        raw: &raw,
        serialized_tail: &serialized_tail,
    };
    assert_resource(
        &value,
        JsonTestLimits::new().with_max_nodes(3),
        JsonResource::Nodes,
    );
    assert_eq!(serialized_tail.get(), 1);
}

/// Verifies raw depth failures stop traversal before the following value.
#[test]
fn test_json_encode_serializer_checks_raw_depth_before_tail() {
    let raw = RawValue::from_string(String::from("[[null]]"))
        .expect("fixture must contain valid raw JSON");
    let serialized_tail = Cell::new(0);
    let value = RawThenCountedTail {
        raw: &raw,
        serialized_tail: &serialized_tail,
    };
    assert_resource(
        &value,
        JsonTestLimits::new().with_max_depth(3),
        JsonResource::Depth,
    );
    assert_eq!(serialized_tail.get(), 0);
}

/// Verifies an impossible raw output size is rejected before raw traversal.
#[test]
fn test_json_encode_serializer_checks_raw_output_lower_bound() {
    let raw = RawValue::from_string(String::from("[null]"))
        .expect("fixture must contain valid raw JSON");
    assert_resource(
        &raw,
        JsonTestLimits::new()
            .with_max_output_bytes(5)
            .with_max_nodes(0),
        JsonResource::OutputBytes,
    );
}

/// Verifies raw output insufficiency precedes the fragment's depth failure.
#[test]
fn test_json_encode_serializer_prioritizes_raw_output_over_depth() {
    let raw = RawValue::from_string(String::from("[null]"))
        .expect("fixture must contain valid raw JSON");
    assert_resource(
        &raw,
        JsonTestLimits::new()
            .with_max_output_bytes(5)
            .with_max_depth(0),
        JsonResource::OutputBytes,
    );
}

/// Verifies raw lower bounds use remaining output in a reused encode session.
#[test]
fn test_json_encode_serializer_checks_raw_output_remaining_in_reused_session() {
    let raw = RawValue::from_string(String::from("[null]"))
        .expect("fixture must contain valid raw JSON");
    let mut session = JsonTestLimits::new()
        .with_max_output_bytes(8)
        .with_max_nodes(1)
        .encode_session();
    assert_eq!(
        encode_to_vec(&true, &mut session)
            .expect("the first document must fit"),
        b"true",
    );
    let output = session
        .output_budget()
        .expect("the test configures output accounting");
    assert_eq!(output.used(), 4);
    assert_eq!(output.remaining(), 4);

    let error = encode_to_vec(&raw, &mut session)
        .expect_err("the raw fragment must exceed live remaining output");

    assert!(matches!(
        error,
        JsonEncodeError::Budget(error)
            if matches!(
                error.budget_error(),
                Some(BudgetError::Insufficient {
                    resource: JsonResource::OutputBytes,
                    limit: 8,
                    remaining: 4,
                    requested: 6,
                })
            )
    ));
    let output = session
        .output_budget()
        .expect("the test configures output accounting");
    assert_eq!(output.used(), 4);
    assert_eq!(output.remaining(), 4);
}

/// Verifies transparent wrappers and every enum shape retain serde_json output.
#[test]
fn test_json_encode_serializer_preserves_wrapper_and_enum_shapes() {
    assert_same_json(&Some(Newtype(7)));
    assert_same_json(&Option::<Newtype>::None);
    assert_same_json(&EnumShape::Unit);
    assert_same_json(&EnumShape::Newtype(1));
    assert_same_json(&EnumShape::Tuple(1, 2));
    assert_same_json(&EnumShape::Struct { value: 3 });

    assert_node_count(&Some(Newtype(7)), 1);
    assert_node_count(&Option::<Newtype>::None, 1);
    assert_node_count(&EnumShape::Unit, 1);
    assert_node_count(&EnumShape::Newtype(1), 2);
    assert_node_count(&EnumShape::Tuple(1, 2), 4);
    assert_node_count(&EnumShape::Struct { value: 3 }, 3);
}
