// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Public strict JSON value encoding contracts.

use std::fmt;

use qubit_json::encode::JsonEncoder;
use qubit_json::value::JsonValueEncodeError;
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

/// Emits one floating-point map key.
struct FloatKeyProbe(f64);

impl Serialize for FloatKeyProbe {
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

/// Reports the stable nested non-finite-float diagnostic.
struct NestedNonFiniteProbe;

impl Serialize for NestedNonFiniteProbe {
    /// Returns the same custom diagnostic used by finite-float adapters.
    fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        Err(SerializeError::custom("non-finite floating-point value"))
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

/// Exercises the recommended encoder construction and successful projection.
#[test]
fn test_json_value_encoder_encodes_nested_value() {
    let encoder = JsonValueEncoder::new();
    let actual = encoder
        .encode(&json!({"items": [1, true, null], "name": "Ada"}))
        .expect("strict JSON-compatible values should encode");
    assert_eq!(actual, json!({"items": [1, true, null], "name": "Ada"}));
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
    assert_eq!(encoder.encode(&i128::MAX), Err(JsonValueEncodeError::Serialization));
    assert_eq!(encoder.encode(&u128::MAX), Err(JsonValueEncodeError::Serialization));
}

/// Classifies direct and nested non-finite floating-point failures.
#[test]
fn test_json_value_encoder_rejects_non_finite_floats() {
    let encoder = JsonValueEncoder::new();
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(encoder.encode(&value), Err(JsonValueEncodeError::NonFiniteFloat));
    }
    assert_eq!(
        encoder.encode(&NestedNonFiniteProbe),
        Err(JsonValueEncodeError::NonFiniteFloat)
    );
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

/// Classifies non-finite map keys as non-finite float failures.
#[test]
fn test_json_value_encoder_rejects_non_finite_float_map_key() {
    assert_eq!(
        JsonValueEncoder::new().encode(&FloatKeyProbe(f64::NAN)),
        Err(JsonValueEncodeError::NonFiniteFloat),
    );
}

/// Rejects object keys that collide after JSON key conversion.
#[test]
fn test_json_value_encoder_rejects_duplicate_object_key() {
    assert_eq!(
        JsonValueEncoder::new().encode(&DuplicateKeyProbe),
        Err(JsonValueEncodeError::Serialization)
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

/// Rejects RawValue numbers outside the strict integer range.
#[test]
fn test_json_value_encoder_rejects_wide_raw_value_number() {
    let raw = RawValue::from_string(String::from("18446744073709551616"))
        .expect("serde_json RawValue accepts syntactically valid wide integers");
    assert_eq!(
        JsonValueEncoder::new().encode(&raw),
        Err(JsonValueEncodeError::Serialization)
    );
}

/// Treats serde_json's former number marker as an ordinary object key.
#[test]
fn test_json_value_encoder_preserves_former_number_marker_object() {
    const MARKER: &str = concat!("$", "serde_json", "::private::Number");
    let source = json!({MARKER: "123"});
    assert_eq!(JsonValueEncoder::new().encode(&source), Ok(source));
}

/// Covers every scalar and compound Serde serializer entry point.
#[test]
fn test_json_value_encoder_supports_all_serde_entry_points() {
    let encoder = JsonValueEncoder::new();
    for index in 0..=30 {
        encoder
            .encode(&SerializerProbe(index))
            .expect("supported Serde entry point should encode");
    }
}
