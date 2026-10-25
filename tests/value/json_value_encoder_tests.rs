// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Public strict JSON value encoding contracts.

use std::cell::Cell;
use std::fmt;

use qubit_json::decode::JsonDecoder;
use qubit_json::encode::JsonEncodeErrorKind;
use qubit_json::encode::JsonEncoder;
use qubit_json::encode::JsonIntegerSignedness;
use qubit_json::encode::JsonMapKeyKind;
use qubit_json::encode::JsonSerializationErrorCategory;
use qubit_json::encode::JsonSerializationErrorKind;
use qubit_json::encode::JsonSerializerStateError;
use qubit_json::value::JsonValueEncoder;
use serde::Serialize;
use serde::Serializer;
use serde::ser::Error as SerializeError;
use serde::ser::SerializeMap;
use serde::ser::SerializeSeq;
use serde::ser::SerializeStruct;
use serde::ser::SerializeStructVariant;
use serde::ser::SerializeTuple;
use serde::ser::SerializeTupleStruct;
use serde::ser::SerializeTupleVariant;
use serde_json::Number;
use serde_json::Value;
use serde_json::from_value;
use serde_json::json;
use serde_json::to_string;
use serde_json::to_vec;
use serde_json::value::RawValue;

const RAW_VALUE_TOKEN: &str = concat!("$", "serde_json", ":", ":private::RawValue");

/// Emits a value under serde_json's private RawValue newtype name.
struct RawValueNameProbe<T>(T);

impl<T: Serialize> Serialize for RawValueNameProbe<T> {
    /// Passes the payload through the private RawValue newtype protocol.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_newtype_struct(RAW_VALUE_TOKEN, &self.0)
    }
}

/// Ordinary derived newtype used to check transparent projection.
#[derive(Serialize)]
#[serde(transparent)]
struct TransparentProbe<T>(T);

/// Emits two entries that become the same JSON object key.
struct DuplicateKeyProbe;

impl Serialize for DuplicateKeyProbe {
    /// Serializes two values under the same key.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(2))?;
        map.serialize_entry("same", &1_u8)?;
        map.serialize_entry("same", &2_u8)?;
        map.end()
    }
}

struct SwallowedNestedFailure;

impl Serialize for SwallowedNestedFailure {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut sequence = serializer.serialize_seq(Some(1))?;
        let _ = sequence.serialize_element(&f64::NAN);
        sequence.end()
    }
}

struct RejectedValueProbe<'a>(&'a Cell<usize>);

impl Serialize for RejectedValueProbe<'_> {
    fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.0.set(self.0.get() + 1);
        Err(S::Error::custom("duplicate value must not be projected"))
    }
}

struct DuplicateProjectionProbe<'a> {
    path: &'static str,
    calls: &'a Cell<usize>,
}

impl Serialize for DuplicateProjectionProbe<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let rejected = RejectedValueProbe(self.calls);
        match self.path {
            "split-map" => {
                let mut map = serializer.serialize_map(Some(2))?;
                map.serialize_entry("same", &1_u8)?;
                map.serialize_key("same")?;
                map.serialize_value(&rejected)?;
                map.end()
            }
            "entry-map" => {
                let mut map = serializer.serialize_map(Some(2))?;
                map.serialize_entry("same", &1_u8)?;
                map.serialize_entry("same", &rejected)?;
                map.end()
            }
            "struct" => {
                let mut object = serializer.serialize_struct("Probe", 2)?;
                object.serialize_field("same", &1_u8)?;
                object.serialize_field("same", &rejected)?;
                object.end()
            }
            "variant" => {
                let mut object = serializer.serialize_struct_variant("Probe", 0, "Object", 2)?;
                object.serialize_field("same", &1_u8)?;
                object.serialize_field("same", &rejected)?;
                object.end()
            }
            _ => Err(S::Error::custom("unknown test path")),
        }
    }
}

/// Declares an empty sequence and then emits one element.
struct ZeroDeclaredSequence;

impl Serialize for ZeroDeclaredSequence {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut sequence = serializer.serialize_seq(Some(0))?;
        sequence.serialize_element(&1_u8)?;
        sequence.end()
    }
}

/// Emits one floating-point map key.
struct FloatKeyProbe<T>(T);

impl<T: Serialize> Serialize for FloatKeyProbe<T> {
    /// Serializes one map entry whose key is not JSON-compatible.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(1))?;
        map.serialize_entry(&self.0, &true)?;
        map.end()
    }
}

/// Selects one Serde serializer entry point for an object key.
struct MapKeyProbe(u8);

impl Serialize for MapKeyProbe {
    /// Emits the selected scalar or compound Serde key shape.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self.0 {
            0 => serializer.serialize_bool(true),
            1 => serializer.serialize_i8(-8),
            2 => serializer.serialize_i16(-16),
            3 => serializer.serialize_i32(-32),
            4 => serializer.serialize_i64(-64),
            5 => serializer.serialize_i128(-128),
            6 => serializer.serialize_u8(8),
            7 => serializer.serialize_u16(16),
            8 => serializer.serialize_u32(32),
            9 => serializer.serialize_u64(64),
            10 => serializer.serialize_u128(128),
            11 => serializer.serialize_f32(1.5),
            12 => serializer.serialize_f64(2.5),
            13 => serializer.serialize_char('x'),
            14 => serializer.serialize_str("text"),
            15 => serializer.serialize_bytes(&[1, 2]),
            16 => serializer.serialize_none(),
            17 => serializer.serialize_some(&1_i32),
            18 => serializer.serialize_unit(),
            19 => serializer.serialize_unit_struct("Unit"),
            20 => serializer.serialize_unit_variant("Enum", 0, "Unit"),
            21 => serializer.serialize_newtype_struct("New", &-1_i32),
            22 => serializer.serialize_newtype_variant("Enum", 0, "New", &1_i32),
            23 => {
                let _ = serializer.serialize_seq(Some(0))?;
                unreachable!("map-key serializer must reject sequences")
            }
            24 => {
                let _ = serializer.serialize_tuple(0)?;
                unreachable!("map-key serializer must reject tuples")
            }
            25 => {
                let _ = serializer.serialize_tuple_struct("Tuple", 0)?;
                unreachable!("map-key serializer must reject tuple structs")
            }
            26 => {
                let _ = serializer.serialize_tuple_variant("Enum", 0, "Tuple", 0)?;
                unreachable!("map-key serializer must reject tuple variants")
            }
            27 => {
                let _ = serializer.serialize_map(Some(0))?;
                unreachable!("map-key serializer must reject maps")
            }
            28 => {
                let _ = serializer.serialize_struct("Object", 0)?;
                unreachable!("map-key serializer must reject structs")
            }
            29 => {
                let _ = serializer.serialize_struct_variant("Enum", 0, "Object", 0)?;
                unreachable!("map-key serializer must reject struct variants")
            }
            30 => serializer.collect_str(&DisplayProbe),
            31 => serializer.serialize_f32(f32::NAN),
            _ => serializer.serialize_f64(f64::NAN),
        }
    }
}

/// Emits one map entry using a selected key representation.
struct MapKeyContainer(u8);

impl Serialize for MapKeyContainer {
    /// Serializes one Boolean value under the selected key shape.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(1))?;
        map.serialize_entry(&MapKeyProbe(self.0), &true)?;
        map.end()
    }
}

/// Selects one boundary or wrapper representation in the unified map-key
/// contract.
enum ContractMapKey {
    /// Full-range signed integer key.
    Signed(i128),
    /// Full-range unsigned integer key.
    Unsigned(u128),
    /// Present option transparently wrapping an integer key.
    Present(i32),
    /// Newtype struct transparently wrapping an integer key.
    Newtype(i32),
}

impl Serialize for ContractMapKey {
    /// Emits the selected key representation through its exact Serde entry
    /// point.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::Signed(value) => serializer.serialize_i128(*value),
            Self::Unsigned(value) => serializer.serialize_u128(*value),
            Self::Present(value) => serializer.serialize_some(value),
            Self::Newtype(value) => serializer.serialize_newtype_struct("Key", value),
        }
    }
}

/// Emits one entry using a boundary or wrapper map key.
struct ContractMapKeyContainer(ContractMapKey);

impl Serialize for ContractMapKeyContainer {
    /// Serializes one Boolean value under the selected contract key.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(1))?;
        map.serialize_entry(&self.0, &true)?;
        map.end()
    }
}

/// Reports text that used to be mistaken for an internal float diagnostic.
struct NestedNonFiniteProbe;

impl Serialize for NestedNonFiniteProbe {
    /// Returns custom text that must remain an opaque custom failure.
    fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        Err(SerializeError::custom("non-finite floating-point value"))
    }
}

/// Display implementation that deliberately rejects formatting.
struct FailingDisplayProbe;

impl fmt::Display for FailingDisplayProbe {
    /// Rejects the formatting request without writing text.
    fn fmt(&self, _formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        Err(fmt::Error)
    }
}

impl Serialize for FailingDisplayProbe {
    /// Exercises the serializer's fallible `collect_str` implementation.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_str(self)
    }
}

/// Selects one invalid split-map call sequence.
struct InvalidMapStateProbe(u8);

impl Serialize for InvalidMapStateProbe {
    /// Violates one Serde map compound-state rule.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(1))?;
        match self.0 {
            0 => map.serialize_value(&true)?,
            1 => {
                map.serialize_key("first")?;
                map.serialize_key("second")?;
            }
            _ => map.serialize_key("pending")?,
        }
        map.end()
    }
}

/// Selects one Serde serializer entry point by index.
struct SerializerProbe(u8);

impl Serialize for SerializerProbe {
    /// Emits the selected scalar or compound Serde shape.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self.0 {
            0 => serializer.serialize_bool(true),
            1 => serializer.serialize_i8(-1),
            2 => serializer.serialize_i16(-1),
            3 => serializer.serialize_i32(-1),
            4 => serializer.serialize_i64(-1),
            5 => serializer.serialize_i128(-1),
            6 => serializer.serialize_u8(1),
            7 => serializer.serialize_u16(1),
            8 => serializer.serialize_u32(1),
            9 => serializer.serialize_u64(1),
            10 => serializer.serialize_u128(1),
            11 => serializer.serialize_f32(1.0),
            12 => serializer.serialize_f64(1.0),
            13 => serializer.serialize_char('x'),
            14 => serializer.serialize_str("x"),
            15 => serializer.serialize_bytes(&[1, 2]),
            16 => serializer.serialize_none(),
            17 => serializer.serialize_some(&1_i32),
            18 => serializer.serialize_unit(),
            19 => serializer.serialize_unit_struct("Unit"),
            20 => serializer.serialize_unit_variant("Enum", 0, "Unit"),
            21 => serializer.serialize_newtype_struct("New", &1_i32),
            22 => serializer.serialize_newtype_variant("Enum", 0, "New", &1_i32),
            23 => {
                let mut sequence = serializer.serialize_seq(Some(1))?;
                sequence.serialize_element(&1_i32)?;
                sequence.end()
            }
            24 => {
                let mut tuple = serializer.serialize_tuple(1)?;
                tuple.serialize_element(&1_i32)?;
                tuple.end()
            }
            25 => {
                let mut tuple = serializer.serialize_tuple_struct("Tuple", 1)?;
                tuple.serialize_field(&1_i32)?;
                tuple.end()
            }
            26 => {
                let mut tuple = serializer.serialize_tuple_variant("Enum", 0, "Tuple", 1)?;
                tuple.serialize_field(&1_i32)?;
                tuple.end()
            }
            27 => {
                let mut map = serializer.serialize_map(Some(1))?;
                map.serialize_entry("key", &1_i32)?;
                map.end()
            }
            28 => {
                let mut object = serializer.serialize_struct("Object", 1)?;
                object.serialize_field("key", &1_i32)?;
                object.end()
            }
            29 => {
                let mut object = serializer.serialize_struct_variant("Enum", 0, "Object", 1)?;
                object.serialize_field("key", &1_i32)?;
                object.end()
            }
            _ => serializer.collect_str(&DisplayProbe),
        }
    }
}

/// Provides a stable collect_str fixture.
struct DisplayProbe;

impl fmt::Display for DisplayProbe {
    /// Writes the expected collected string.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("display")
    }
}

#[derive(Serialize)]
struct UnitStruct;

#[derive(Serialize)]
struct Coordinate(i32, bool);

#[derive(Serialize)]
struct NamedRecord {
    id: u32,
    active: bool,
}

#[derive(Serialize)]
struct OptionalRecord {
    name: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    note: Option<&'static str>,
}

#[derive(Serialize)]
enum CompoundVariant {
    Unit,
    Newtype(i32),
    Tuple(i32, bool),
    Struct { id: u32, active: bool },
}

/// Exercises Serde's explicit skipped-field callback for struct serializers.
struct ExplicitSkipFieldProbe;

impl Serialize for ExplicitSkipFieldProbe {
    /// Skips one field before serializing the retained field.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut record = serializer.serialize_struct("Record", 2)?;
        record.skip_field("omitted")?;
        record.serialize_field("kept", &7_u8)?;
        record.end()
    }
}

/// Exercises the recommended encoder construction and successful projection.
#[test]
fn test_json_value_encoder_encodes_nested_value() {
    let encoder = JsonValueEncoder::new();
    let actual = encoder
        .encode(&json!({"items": [1, true, null], "name": "Ada"}))
        .expect("strict JSON-compatible values should encode");
    assert_eq!(actual, json!({"items": [1, true, null], "name": "Ada"}));
}

#[test]
fn test_json_value_encoder_preserves_swallowed_nested_error() {
    let error = JsonValueEncoder::new()
        .encode(&SwallowedNestedFailure)
        .expect_err("a compound must retain an ignored nested serialization error");

    assert_eq!(error.kind(), JsonSerializationErrorKind::NonFiniteFloat);
}

/// Preserves the complete signed and unsigned 64-bit integer range.
#[test]
fn test_json_value_encoder_preserves_integer_boundaries() {
    let encoder = JsonValueEncoder::default();
    assert_eq!(encoder.encode(&i64::MIN), Ok(json!(i64::MIN)));
    assert_eq!(encoder.encode(&u64::MAX), Ok(json!(u64::MAX)));
    assert_eq!(encoder.encode(&(i64::MIN as i128)), Ok(json!(i64::MIN)));
    assert_eq!(encoder.encode(&(u64::MAX as u128)), Ok(json!(u64::MAX)));
}

/// Rejects integers outside the strict 64-bit JSON representation range.
#[test]
fn test_json_value_encoder_rejects_wide_integers() {
    let encoder = JsonValueEncoder::new();
    let signed = encoder.encode(&i128::MAX).expect_err("wide signed integer must fail");
    let unsigned = encoder.encode(&u128::MAX).expect_err("wide unsigned integer must fail");
    assert_eq!(
        signed.kind(),
        JsonSerializationErrorKind::IntegerOutOfRange {
            signedness: JsonIntegerSignedness::Signed,
        }
    );
    assert_eq!(signed.category(), JsonSerializationErrorCategory::Number);
    assert!(signed.is_number_error());
    assert_eq!(signed.integer_signedness(), Some(JsonIntegerSignedness::Signed));
    assert_eq!(
        unsigned.kind(),
        JsonSerializationErrorKind::IntegerOutOfRange {
            signedness: JsonIntegerSignedness::Unsigned,
        }
    );
}

/// Classifies direct and nested non-finite floating-point failures.
#[test]
fn test_json_value_encoder_rejects_non_finite_floats() {
    let encoder = JsonValueEncoder::new();
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let error = encoder.encode(&value).expect_err("non-finite float must fail");
        assert_eq!(error.kind(), JsonSerializationErrorKind::NonFiniteFloat);
    }
    let custom = encoder
        .encode(&NestedNonFiniteProbe)
        .expect_err("custom serializer failure must remain custom");
    assert_eq!(custom.kind(), JsonSerializationErrorKind::CustomSerialization);
    assert_eq!(custom.category(), JsonSerializationErrorCategory::Custom);
    assert!(!custom.to_string().contains("non-finite floating-point value"));
}

/// Keeps f32 serialization short enough to round-trip as the original f32.
#[test]
fn test_json_value_encoder_preserves_float32_semantics() {
    let encoder = JsonValueEncoder::new();
    let source = f32::from_bits(0xC65B_9806);
    let projected = encoder.encode(&source).expect("finite f32 should encode");
    let text = to_string(&projected).expect("encoded JSON value should render");
    let widened = Value::Number(Number::from_f64(f64::from(source)).expect("source is finite"));
    assert_eq!(
        from_value::<f32>(projected).expect("projected f32 should decode"),
        source
    );
    assert_ne!(text, to_string(&widened).expect("widened number should render"));
}

#[test]
fn test_json_value_encoder_f32_keys_match_direct_encoding() {
    for key in [
        1.0_f32,
        -1.0,
        0.0,
        -0.0,
        1.5,
        1e20,
        -1e20,
        f32::MIN_POSITIVE,
        f32::from_bits(1),
        f32::MAX,
    ] {
        let probe = FloatKeyProbe(key);
        let projected = JsonValueEncoder::new().encode(&probe).unwrap();
        let direct = JsonEncoder::unlimited().to_vec(&probe).unwrap();
        assert_eq!(to_vec(&projected).unwrap(), direct, "key bits={:08x}", key.to_bits());
    }
}

#[test]
fn test_json_value_encoder_f32_values_remain_strictly_encodable() {
    for source in [
        1.0_f32,
        -1.0,
        0.0,
        -0.0,
        1.5,
        1e20,
        -1e20,
        f32::MIN_POSITIVE,
        f32::from_bits(1),
        f32::MAX,
        f32::from_bits(0xC65B_9806),
    ] {
        let direct = JsonEncoder::unlimited().to_vec(&source).unwrap();
        let expected: Value = JsonDecoder::unlimited().decode_utf8(&direct).unwrap();
        let projected = JsonValueEncoder::new().encode(&source).unwrap();
        assert_eq!(projected, expected, "value bits={:08x}", source.to_bits());
        let encoded = JsonEncoder::unlimited().to_vec(&projected).unwrap();
        let _: Value = JsonDecoder::unlimited().decode_utf8(&encoded).unwrap();
        assert_eq!(
            from_value::<f32>(projected).unwrap().to_bits(),
            source.to_bits(),
            "f32 round trip must retain the original bits",
        );
    }
}

#[test]
fn test_json_value_encoder_f32_rejects_non_finite_values_and_keys() {
    for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let error = JsonValueEncoder::new().encode(&value).unwrap_err();
        assert_eq!(error.kind(), JsonSerializationErrorKind::NonFiniteFloat);
        let error = JsonValueEncoder::new().encode(&FloatKeyProbe(value)).unwrap_err();
        assert_eq!(error.kind(), JsonSerializationErrorKind::NonFiniteFloat);
    }
}

/// Accepts finite map keys using the same text as the strict text encoder.
#[test]
fn test_json_value_encoder_encodes_finite_float_map_key() {
    assert_eq!(
        JsonValueEncoder::new().encode(&FloatKeyProbe(1.5)),
        Ok(json!({"1.5": true})),
    );
}

/// Uses the same finite floating-point key text as strict JSON text encoding.
#[test]
fn test_json_value_encoder_matches_text_encoder_float_map_keys() {
    let value_encoder = JsonValueEncoder::new();
    let mut text_encoder = JsonEncoder::unlimited();
    for key in [-0.0, 1.0, 1.5, f64::MIN_POSITIVE, f64::MAX] {
        let probe = FloatKeyProbe(key);
        let value = value_encoder.encode(&probe).expect("finite key should encode");
        let value_text = to_vec(&value).expect("JSON value should render");
        let direct_text = text_encoder.to_vec(&probe).expect("finite key should encode as text");

        assert_eq!(value_text, direct_text, "map key differs for {key}");
    }
}

/// Uses one contract for wide integer and transparent wrapper map keys across
/// both strict encoding entry points.
#[test]
fn test_json_value_encoder_matches_text_encoder_wide_and_wrapped_map_keys() {
    let cases = [
        (ContractMapKey::Signed(i128::MIN), i128::MIN.to_string()),
        (ContractMapKey::Signed(i128::MAX), i128::MAX.to_string()),
        (ContractMapKey::Unsigned(u128::MAX), u128::MAX.to_string()),
        (ContractMapKey::Present(-7), String::from("-7")),
        (ContractMapKey::Newtype(9), String::from("9")),
    ];
    let value_encoder = JsonValueEncoder::new();
    let mut text_encoder = JsonEncoder::unlimited();

    for (key, expected_key) in cases {
        let probe = ContractMapKeyContainer(key);
        let value = value_encoder
            .encode(&probe)
            .expect("contract map key should encode as a JSON value");
        let value_text = to_vec(&value).expect("encoded JSON value should render");
        let direct_text = text_encoder
            .to_vec(&probe)
            .expect("contract map key should encode as strict text");

        assert_eq!(value, json!({expected_key: true}));
        assert_eq!(value_text, direct_text, "map-key entry points must agree");
    }
}

/// Classifies non-finite map keys as non-finite float failures.
#[test]
fn test_json_value_encoder_rejects_non_finite_float_map_key() {
    let error = JsonValueEncoder::new()
        .encode(&FloatKeyProbe(f64::NAN))
        .expect_err("non-finite map key must fail");
    assert_eq!(error.kind(), JsonSerializationErrorKind::NonFiniteFloat);
}

/// Covers every scalar key representation accepted by strict value encoding.
#[test]
fn test_json_value_encoder_supports_scalar_map_key_entry_points() {
    const EXPECTED_KEYS: [&str; 19] = [
        "true", "-8", "-16", "-32", "-64", "-128", "8", "16", "32", "64", "128", "1.5", "2.5", "x", "text", "1",
        "Unit", "-1", "display",
    ];
    let encoder = JsonValueEncoder::new();
    let supported_indices = (0_u8..=14).chain([17, 20, 21, 30]);

    for (index, expected_key) in supported_indices.zip(EXPECTED_KEYS) {
        let encoded = encoder
            .encode(&MapKeyContainer(index))
            .expect("supported scalar map key should encode");
        assert_eq!(
            encoded,
            json!({expected_key: true}),
            "unexpected map key for entry point {index}"
        );
    }
}

/// Rejects every compound or wrapped key representation unsupported by JSON.
#[test]
fn test_json_value_encoder_rejects_unsupported_map_key_entry_points() {
    let encoder = JsonValueEncoder::new();
    let unsupported_indices = (15_u8..=16).chain(18..=19).chain(22..=29);

    for index in unsupported_indices {
        let error = encoder
            .encode(&MapKeyContainer(index))
            .expect_err("unsupported entry point must not produce a JSON object key");
        assert!(matches!(
            error.kind(),
            JsonSerializationErrorKind::UnsupportedMapKey { .. }
        ));
        assert_eq!(error.category(), JsonSerializationErrorCategory::ObjectKey);
        assert!(error.is_map_key_error());
        assert!(
            error.map_key_kind().is_some(),
            "missing map-key kind for entry point {index}"
        );
    }
    assert_eq!(
        encoder
            .encode(&MapKeyContainer(15))
            .expect_err("byte key must fail")
            .map_key_kind(),
        Some(JsonMapKeyKind::Bytes)
    );
    assert_eq!(
        encoder
            .encode(&MapKeyContainer(23))
            .expect_err("sequence key must fail")
            .map_key_kind(),
        Some(JsonMapKeyKind::Sequence)
    );
}

/// Classifies non-finite map keys consistently for both float widths.
#[test]
fn test_json_value_encoder_rejects_non_finite_map_key_entry_points() {
    let encoder = JsonValueEncoder::new();
    for index in [31, 32] {
        let error = encoder
            .encode(&MapKeyContainer(index))
            .expect_err("non-finite map key must fail");
        assert_eq!(
            error.kind(),
            JsonSerializationErrorKind::NonFiniteFloat,
            "entry point {index} must preserve the non-finite classification",
        );
    }
}

/// Rejects object keys that collide after JSON key conversion.
#[test]
fn test_json_value_encoder_rejects_duplicate_object_key() {
    let error = JsonValueEncoder::new()
        .encode(&DuplicateKeyProbe)
        .expect_err("duplicate key must fail");
    assert_eq!(error.kind(), JsonSerializationErrorKind::DuplicateObjectKey);
    assert_eq!(error.category(), JsonSerializationErrorCategory::ObjectKey);
}

#[test]
fn test_json_value_encoder_rejects_duplicate_before_value_projection() {
    for path in ["split-map", "entry-map", "struct", "variant"] {
        let calls = Cell::new(0);
        let error = JsonValueEncoder::new()
            .encode(&DuplicateProjectionProbe { path, calls: &calls })
            .expect_err("duplicate key must fail");
        assert_eq!(calls.get(), 0, "duplicate value was projected via {path}");
        assert_eq!(error.kind(), JsonSerializationErrorKind::DuplicateObjectKey);
    }
}

#[test]
fn test_json_value_encoder_preserves_distinct_object_values() {
    assert_eq!(
        JsonValueEncoder::new()
            .encode(&json!({"first": 1, "second": [2, 3]}))
            .unwrap(),
        json!({"first": 1, "second": [2, 3]})
    );
}

/// Materializes a strict RawValue payload into its represented JSON value.
#[test]
fn test_json_value_encoder_materializes_raw_value() {
    let raw =
        RawValue::from_string(String::from(r#"{"ok":true,"values":[1,2]}"#)).expect("fixture should be valid raw JSON");
    let actual = JsonValueEncoder::new()
        .encode(&raw)
        .expect("strict RawValue should materialize");
    assert_eq!(actual, json!({"ok": true, "values": [1, 2]}));
}

/// Keeps ordinary and private RawValue newtypes on their intended paths.
#[test]
fn test_json_value_encoder_newtype_delegation_and_raw_protocol() {
    let encoder = JsonValueEncoder::new();
    assert_eq!(encoder.encode(&TransparentProbe(Some(true))), Ok(json!(true)));

    let raw_name = RawValueNameProbe(String::from("true"));
    assert_eq!(encoder.encode(&raw_name), Ok(json!("true")));

    let raw_bool = RawValueNameProbe(true);
    assert_eq!(encoder.encode(&raw_bool), Ok(json!(true)));

    let raw_option = RawValueNameProbe(Some(String::from("true")));
    assert_eq!(encoder.encode(&raw_option), Ok(json!("true")));

    let mut ordinary = JsonEncoder::unlimited();
    assert_eq!(ordinary.to_vec(&TransparentProbe(Some(true))).unwrap(), b"true");
    assert_eq!(ordinary.to_vec(&raw_name).unwrap(), br#""true""#);
    assert_eq!(ordinary.to_vec(&raw_bool).unwrap(), b"true");
    assert_eq!(ordinary.to_vec(&raw_option).unwrap(), br#""true""#);
}

/// Enforces the same fixed RawValue materialization boundary at 127/128.
#[test]
fn test_json_value_encoder_raw_value_depth_boundary() {
    let encoder = JsonValueEncoder::new();
    for depth in [127, 128] {
        let text = format!("{}null{}", "[".repeat(depth), "]".repeat(depth));
        let raw = RawValue::from_string(text).expect("nested fixture is valid JSON");
        let result = encoder.encode(&raw);
        if depth == 127 {
            let mut value = json!(null);
            for _ in 0..127 {
                value = json!([value]);
            }
            assert_eq!(result, Ok(value));
        } else {
            let error = result.expect_err("128 nested containers exceed the projection limit");
            assert_eq!(error.kind(), JsonSerializationErrorKind::InvalidRawValue);
            assert!(error.to_string().contains("127"));
        }
    }
}

/// Materializes real RawValue numbers as numbers rather than private marker
/// maps.
#[test]
fn test_json_value_encoder_materializes_raw_value_numbers() {
    for (text, expected) in [
        ("1.5", serde_json::json!(1.5)),
        (r#"{"x":1.5}"#, serde_json::json!({"x": 1.5})),
    ] {
        let raw = RawValue::from_string(text.to_owned()).expect("RawValue accepts valid JSON");
        let value = JsonValueEncoder::new()
            .encode(&raw)
            .expect("valid RawValue numbers should materialize");
        assert_eq!(value, expected);
    }
}

/// Rejects RawValue numbers outside the strict integer range.
#[test]
fn test_json_value_encoder_rejects_wide_raw_value_number() {
    let raw = RawValue::from_string(String::from("18446744073709551616"))
        .expect("serde_json RawValue accepts syntactically valid wide integers");
    let error = JsonValueEncoder::new()
        .encode(&raw)
        .expect_err("wide RawValue number must fail");
    assert_eq!(error.kind(), JsonSerializationErrorKind::InvalidRawValue);
    assert_eq!(error.category(), JsonSerializationErrorCategory::RawValue);
    assert!(error.is_raw_value_error());
}

/// Converts display-formatting failures into a stable error instead of
/// panicking.
#[test]
fn test_json_value_encoder_classifies_display_formatting_failure() {
    let error = JsonValueEncoder::new()
        .encode(&FailingDisplayProbe)
        .expect_err("failing Display must produce an error");
    assert_eq!(error.kind(), JsonSerializationErrorKind::DisplayFormattingFailed);
    assert_eq!(error.category(), JsonSerializationErrorCategory::SerializerContract);
}

/// Classifies each externally reachable invalid map call sequence.
#[test]
fn test_json_value_encoder_classifies_invalid_map_states() {
    let encoder = JsonValueEncoder::new();
    let expected = [
        JsonSerializerStateError::MapValueWithoutKey,
        JsonSerializerStateError::MapKeyAlreadyPending,
        JsonSerializerStateError::MapEndedWithPendingKey,
    ];
    for (index, reason) in expected.into_iter().enumerate() {
        let error = encoder
            .encode(&InvalidMapStateProbe(index as u8))
            .expect_err("invalid map state must fail");
        assert_eq!(
            error.kind(),
            JsonSerializationErrorKind::InvalidSerializerState { reason }
        );
        assert!(error.is_serializer_contract_error());
        assert_eq!(error.serializer_state_error(), Some(reason));
    }
}

/// Treats serde_json's former number marker as an ordinary object key.
#[test]
fn test_json_value_encoder_preserves_former_number_marker_object() {
    const MARKER: &str = concat!("$", "serde_json", "::private::Number");
    let source = json!({MARKER: "123"});
    assert_eq!(JsonValueEncoder::new().encode(&source), Ok(source));
}

/// Asserts the encoded value for every scalar and compound Serde entry point.
#[test]
fn test_json_value_encoder_supports_all_serde_entry_points() {
    let expected = [
        json!(true),
        json!(-1_i64),
        json!(-1_i64),
        json!(-1_i64),
        json!(-1_i64),
        json!(-1_i64),
        json!(1_u64),
        json!(1_u64),
        json!(1_u64),
        json!(1_u64),
        json!(1_u64),
        json!(1.0_f64),
        json!(1.0_f64),
        json!("x"),
        json!("x"),
        json!([1, 2]),
        json!(null),
        json!(1),
        json!(null),
        json!(null),
        json!("Unit"),
        json!(1),
        json!({"New": 1}),
        json!([1]),
        json!([1]),
        json!([1]),
        json!({"Tuple": [1]}),
        json!({"key": 1}),
        json!({"key": 1}),
        json!({"Object": {"key": 1}}),
        json!("display"),
    ];
    assert_eq!(expected.len(), 31, "every probe index must have an expectation");
    let encoder = JsonValueEncoder::new();
    for (index, expected) in expected.into_iter().enumerate() {
        let actual = encoder
            .encode(&SerializerProbe(u8::try_from(index).expect("probe index fits u8")))
            .expect("supported Serde entry point should encode");
        assert_eq!(actual, expected, "Serde entry point {index} emitted the wrong Value");
    }
}

/// Checks the JSON shapes emitted by derived unit, tuple, newtype, and enum
/// serializers.
#[test]
fn test_json_value_encoder_materializes_derived_compound_shapes() {
    let encoder = JsonValueEncoder::new();

    assert_eq!(encoder.encode(&UnitStruct), Ok(Value::Null));
    assert_eq!(encoder.encode(&(7_i32, true)), Ok(json!([7, true])));
    assert_eq!(encoder.encode(&Coordinate(8, false)), Ok(json!([8, false])));
    assert_eq!(
        encoder.encode(&NamedRecord { id: 9, active: true }),
        Ok(json!({"id": 9, "active": true})),
    );
    assert_eq!(
        encoder.encode(&OptionalRecord {
            name: "Ada",
            note: None,
        }),
        Ok(json!({"name": "Ada"})),
    );
    assert_eq!(
        encoder.encode(&OptionalRecord {
            name: "Ada",
            note: Some("memo"),
        }),
        Ok(json!({"name": "Ada", "note": "memo"})),
    );

    assert_eq!(encoder.encode(&CompoundVariant::Unit), Ok(json!("Unit")));
    assert_eq!(
        encoder.encode(&CompoundVariant::Newtype(10)),
        Ok(json!({"Newtype": 10}))
    );
    assert_eq!(
        encoder.encode(&CompoundVariant::Tuple(11, false)),
        Ok(json!({"Tuple": [11, false]})),
    );
    assert_eq!(
        encoder.encode(&CompoundVariant::Struct { id: 12, active: true }),
        Ok(json!({"Struct": {"id": 12, "active": true}})),
    );
}

/// Accepts the Serde struct skip-field callback and preserves remaining fields.
#[test]
fn test_json_value_encoder_accepts_explicit_struct_skip_field() {
    let actual = JsonValueEncoder::new()
        .encode(&ExplicitSkipFieldProbe)
        .expect("a skipped struct field should not alter the object");

    assert_eq!(actual, json!({"kept": 7}));
}

#[test]
fn test_zero_declared_sequence_contract_differs_between_encoders() {
    let value = JsonValueEncoder::new()
        .encode(&ZeroDeclaredSequence)
        .expect("the value encoder uses the declared length as a capacity hint");
    assert_eq!(value, json!([1]));

    let error = JsonEncoder::unlimited()
        .to_vec(&ZeroDeclaredSequence)
        .expect_err("the text encoder rejects elements beyond the declared length");
    assert_eq!(error.kind(), JsonEncodeErrorKind::Serialize);
    assert_eq!(
        error.serialization_error().unwrap().kind(),
        JsonSerializationErrorKind::InvalidSerializerState {
            reason: JsonSerializerStateError::UnexpectedCompound,
        },
    );
}
