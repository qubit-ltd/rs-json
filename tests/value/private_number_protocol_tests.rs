// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Verifies private Number protocol handling in the Value encoder.

use std::collections::BTreeMap;
use std::fmt;

use qubit_json::decode::JsonDecoder;
use qubit_json::encode::JsonSerializationErrorKind;
use qubit_json::encode::JsonSerializerStateError;
use qubit_json::value::JsonValueEncoder;
use serde::Serialize;
use serde::Serializer;
use serde::ser::SerializeStruct;
use serde_json::Number;
use serde_json::Value;
use serde_json::json;
use serde_json::value::RawValue;

/// serde_json's private struct name for number payloads.
const NUMBER: &str = concat!("$", "serde_json", ":", ":private::Number");
/// serde_json's private struct name for raw JSON payloads.
const RAW: &str = concat!("$", "serde_json", ":", ":private::RawValue");

/// Serde fixture that emits a configurable private Number protocol.
#[derive(Clone, Copy)]
struct NumberProtocol {
    /// Declared struct field count supplied to Serde.
    declared_len: usize,
    /// Number of field callbacks to emit.
    fields: usize,
    /// Private protocol field token supplied to each callback.
    field: &'static str,
    /// Text payload supplied for each callback.
    payload: &'static str,
}

impl Serialize for NumberProtocol {
    /// Emits the configured private Number struct protocol.
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

/// Verifies valid number text is materialized and malformed text is rejected.
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
        Ok(Value::Number(Number::from(u64::MAX))),
    );
    assert_eq!(JsonValueEncoder::new().encode(&Number::from(42)), Ok(json!(42)),);

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
            JsonSerializationErrorKind::NonFiniteFloat | JsonSerializationErrorKind::InvalidNumberRepresentation
        ));
    }
}

#[test]
fn test_private_number_zero_exponents_and_underflow() {
    for (text, expected) in [
        ("0e1", 0.0_f64),
        ("0e+10", 0.0),
        ("0.0e-10", 0.0),
        ("-0e1", -0.0),
        ("-0.0e+10", -0.0),
        ("1e-400", 0.0),
        ("-1e-400", -0.0),
        ("5e-324", f64::from_bits(1)),
        ("-5e-324", -f64::from_bits(1)),
    ] {
        let protocol = NumberProtocol {
            payload: text,
            ..valid_probe()
        };
        let value = JsonValueEncoder::new().encode(&protocol).expect(text);
        assert_eq!(value.as_f64().expect(text).to_bits(), expected.to_bits(), "{text}");

        let raw = RawValue::from_string(text.to_owned()).expect(text);
        let raw_value = JsonValueEncoder::new().encode(&raw).expect(text);
        let decoded = JsonDecoder::unlimited().decode_str::<Value>(text).expect(text);
        assert_eq!(
            raw_value.as_f64().expect(text).to_bits(),
            decoded.as_f64().expect(text).to_bits(),
            "{text}"
        );
    }
}

/// Verifies protocol state validation rejects wrong shape while ordinary map
/// keys that resemble private protocol names remain data.
#[test]
fn test_private_number_protocol_rejects_wrong_shapes_and_keeps_lookalike_map_keys() {
    let wrong_field = NumberProtocol {
        field: RAW,
        ..valid_probe()
    };
    let error = JsonValueEncoder::new()
        .encode(&wrong_field)
        .expect_err("wrong field must fail");
    assert_eq!(
        error.serializer_state_error(),
        Some(JsonSerializerStateError::InvalidNumberProtocol),
    );

    let no_field = NumberProtocol {
        fields: 0,
        ..valid_probe()
    };
    let error = JsonValueEncoder::new()
        .encode(&no_field)
        .expect_err("missing field must fail");
    assert_eq!(
        error.serializer_state_error(),
        Some(JsonSerializerStateError::InvalidNumberProtocol),
    );

    let multiple_fields = NumberProtocol {
        fields: 2,
        ..valid_probe()
    };
    let error = JsonValueEncoder::new()
        .encode(&multiple_fields)
        .expect_err("multiple fields must fail");
    assert_eq!(
        error.serializer_state_error(),
        Some(JsonSerializerStateError::InvalidNumberProtocol),
    );

    let wrong_length = NumberProtocol {
        declared_len: 2,
        ..valid_probe()
    };
    let error = JsonValueEncoder::new()
        .encode(&wrong_length)
        .expect_err("wrong declared length must fail");
    assert_eq!(
        error.serializer_state_error(),
        Some(JsonSerializerStateError::InvalidNumberProtocol),
    );

    let mut object = BTreeMap::new();
    object.insert(NUMBER.to_owned(), 1);
    assert_eq!(JsonValueEncoder::new().encode(&object), Ok(json!({ NUMBER: 1 })),);
}

/// Serde fixture that supplies a numeric value where private Number expects
/// text.
struct NonTextNumberPayload;

impl Serialize for NonTextNumberPayload {
    /// Emits a number payload directly to exercise private protocol type
    /// checks.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct(NUMBER, 1)?;
        state.serialize_field(NUMBER, &7_u64)?;
        state.end()
    }
}

/// Emits private Number text through Serde's Display collector.
struct DisplayNumberPayload(&'static str);

impl Serialize for DisplayNumberPayload {
    /// Formats the number text through the private protocol's `collect_str`
    /// path.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct(NUMBER, 1)?;
        state.serialize_field(NUMBER, &DisplayText(self.0))?;
        state.end()
    }
}

/// Text wrapper that Serde serializes using `collect_str`.
struct DisplayText(&'static str);

impl Serialize for DisplayText {
    /// Sends display output to the target serializer.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_str(&self.0)
    }
}

/// Display implementation that refuses formatting writes.
struct FailingDisplay;

impl fmt::Display for FailingDisplay {
    /// Returns a formatting error to exercise the private serializer's failure
    /// mapping.
    fn fmt(&self, _: &mut fmt::Formatter<'_>) -> fmt::Result {
        Err(fmt::Error)
    }
}

/// Emits a Display value whose formatter returns an error.
struct FailingDisplayNumberPayload;

impl Serialize for FailingDisplayNumberPayload {
    /// Sends the failing Display value through the private Number protocol.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct(NUMBER, 1)?;
        state.serialize_field(NUMBER, &FailingDisplayPayload)?;
        state.end()
    }
}

/// Serde wrapper for the failing Display implementation.
struct FailingDisplayPayload;

impl Serialize for FailingDisplayPayload {
    /// Sends the formatter to the target serializer's `collect_str` method.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_str(&FailingDisplay)
    }
}

/// Verifies the private Number protocol rejects non-text payloads.
#[test]
fn test_private_number_value_rejects_non_text_payload() {
    let error = JsonValueEncoder::new()
        .encode(&NonTextNumberPayload)
        .expect_err("numeric payload must be carried as text");
    assert_eq!(
        error.serializer_state_error(),
        Some(JsonSerializerStateError::InvalidNumberProtocol),
    );
}

/// Verifies private Number accepts formatted text and maps formatter failures.
#[test]
fn test_private_number_protocol_collects_display_text() {
    let value = DisplayNumberPayload("1.5");
    assert_eq!(JsonValueEncoder::new().encode(&value), Ok(json!(1.5)),);

    let error = JsonValueEncoder::new()
        .encode(&FailingDisplayNumberPayload)
        .expect_err("formatter failure must fail private Number serialization");
    assert_eq!(error.kind(), JsonSerializationErrorKind::DisplayFormattingFailed,);
}

/// Serde payload shapes that the private Number text serializer must reject.
#[derive(Clone, Copy)]
enum NonTextShape {
    Bool,
    I8,
    I16,
    I32,
    I64,
    I128,
    U8,
    U16,
    U32,
    U64,
    U128,
    F32,
    F64,
    Char,
    Bytes,
    None,
    Some,
    Unit,
    UnitStruct,
    UnitVariant,
    NewtypeStruct,
    NewtypeVariant,
    Sequence,
    Tuple,
    TupleStruct,
    TupleVariant,
    Map,
    Struct,
    StructVariant,
}

/// Emits each unsupported Serde shape through serde_json's private Number
/// protocol.
struct NonTextShapePayload(NonTextShape);

impl Serialize for NonTextShapePayload {
    /// Sends the selected non-text shape to the private Number payload
    /// serializer.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct(NUMBER, 1)?;
        state.serialize_field(NUMBER, &NonTextPayload(self.0))?;
        state.end()
    }
}

/// Dispatches a selected shape to its corresponding Serde serializer method.
struct NonTextPayload(NonTextShape);

impl Serialize for NonTextPayload {
    /// Calls the selected serializer method; the private text serializer
    /// rejects every case.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self.0 {
            NonTextShape::Bool => serializer.serialize_bool(true),
            NonTextShape::I8 => serializer.serialize_i8(1),
            NonTextShape::I16 => serializer.serialize_i16(1),
            NonTextShape::I32 => serializer.serialize_i32(1),
            NonTextShape::I64 => serializer.serialize_i64(1),
            NonTextShape::I128 => serializer.serialize_i128(1),
            NonTextShape::U8 => serializer.serialize_u8(1),
            NonTextShape::U16 => serializer.serialize_u16(1),
            NonTextShape::U32 => serializer.serialize_u32(1),
            NonTextShape::U64 => serializer.serialize_u64(1),
            NonTextShape::U128 => serializer.serialize_u128(1),
            NonTextShape::F32 => serializer.serialize_f32(1.0),
            NonTextShape::F64 => serializer.serialize_f64(1.0),
            NonTextShape::Char => serializer.serialize_char('x'),
            NonTextShape::Bytes => serializer.serialize_bytes(b"x"),
            NonTextShape::None => serializer.serialize_none(),
            NonTextShape::Some => serializer.serialize_some(&1),
            NonTextShape::Unit => serializer.serialize_unit(),
            NonTextShape::UnitStruct => serializer.serialize_unit_struct("Unit"),
            NonTextShape::UnitVariant => serializer.serialize_unit_variant("Enum", 0, "Unit"),
            NonTextShape::NewtypeStruct => serializer.serialize_newtype_struct("Newtype", &1),
            NonTextShape::NewtypeVariant => serializer.serialize_newtype_variant("Enum", 0, "Newtype", &1),
            NonTextShape::Sequence => serializer.serialize_seq(Some(0)).map(|_| unreachable!()),
            NonTextShape::Tuple => serializer.serialize_tuple(0).map(|_| unreachable!()),
            NonTextShape::TupleStruct => serializer.serialize_tuple_struct("Tuple", 0).map(|_| unreachable!()),
            NonTextShape::TupleVariant => serializer
                .serialize_tuple_variant("Enum", 0, "Tuple", 0)
                .map(|_| unreachable!()),
            NonTextShape::Map => serializer.serialize_map(Some(0)).map(|_| unreachable!()),
            NonTextShape::Struct => serializer.serialize_struct("Struct", 0).map(|_| unreachable!()),
            NonTextShape::StructVariant => serializer
                .serialize_struct_variant("Enum", 0, "Struct", 0)
                .map(|_| unreachable!()),
        }
    }
}

/// Verifies every non-text serializer shape is rejected by the Number
/// protocol with its stable state error.
#[test]
fn test_private_number_protocol_rejects_non_text_serde_shapes() {
    let shapes = [
        NonTextShape::Bool,
        NonTextShape::I8,
        NonTextShape::I16,
        NonTextShape::I32,
        NonTextShape::I64,
        NonTextShape::I128,
        NonTextShape::U8,
        NonTextShape::U16,
        NonTextShape::U32,
        NonTextShape::U64,
        NonTextShape::U128,
        NonTextShape::F32,
        NonTextShape::F64,
        NonTextShape::Char,
        NonTextShape::Bytes,
        NonTextShape::None,
        NonTextShape::Some,
        NonTextShape::Unit,
        NonTextShape::UnitStruct,
        NonTextShape::UnitVariant,
        NonTextShape::NewtypeStruct,
        NonTextShape::NewtypeVariant,
        NonTextShape::Sequence,
        NonTextShape::Tuple,
        NonTextShape::TupleStruct,
        NonTextShape::TupleVariant,
        NonTextShape::Map,
        NonTextShape::Struct,
        NonTextShape::StructVariant,
    ];

    for shape in shapes {
        let error = JsonValueEncoder::new()
            .encode(&NonTextShapePayload(shape))
            .expect_err("non-text private Number shape must fail");
        assert_eq!(
            error.serializer_state_error(),
            Some(JsonSerializerStateError::InvalidNumberProtocol),
        );
    }
}

/// Creates a valid one-field Number protocol fixture for mutation by tests.
fn valid_probe() -> NumberProtocol {
    NumberProtocol {
        declared_len: 1,
        fields: 1,
        field: NUMBER,
        payload: "1.5",
    }
}

mod projection_lifecycle {
    use qubit_json::encode::JsonSerializerStateError;
    use qubit_json::value::JsonValueEncoder;
    use serde::Serialize;
    use serde::ser::SerializeMap;
    use serde::ser::SerializeStruct;

    use super::NUMBER;
    use super::RAW;

    struct ValidPrivateNumber;

    impl Serialize for ValidPrivateNumber {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            let mut protocol = serializer.serialize_struct(NUMBER, 1)?;
            protocol.serialize_field(NUMBER, "17")?;
            protocol.end()
        }
    }

    struct InvalidPrivateNumber;

    impl Serialize for InvalidPrivateNumber {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            let mut protocol = serializer.serialize_struct(NUMBER, 1)?;
            protocol.serialize_field(NUMBER, "not-a-number")?;
            protocol.end()
        }
    }

    struct InvalidPrivateNumberField;

    impl Serialize for InvalidPrivateNumberField {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            let mut protocol = serializer.serialize_struct(NUMBER, 1)?;
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
            let protocol = serializer.serialize_struct(NUMBER, 1)?;
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
            let protocol = serializer.serialize_struct(RAW, 1)?;
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
            Some(JsonSerializerStateError::MapEndedWithPendingKey)
        );

        let private_error = JsonValueEncoder::new()
            .encode(&IgnoresPrivateProtocolEndFailure)
            .unwrap_err();
        assert_eq!(
            private_error.serializer_state_error(),
            Some(JsonSerializerStateError::InvalidNumberProtocol)
        );
    }

    #[test]
    fn test_projection_rejects_incomplete_private_raw_value_protocol() {
        let error = JsonValueEncoder::new().encode(&MissingPrivateRawValue).unwrap_err();
        assert!(error.is_serializer_contract_error());
        assert_eq!(
            error.serializer_state_error(),
            Some(JsonSerializerStateError::InvalidRawValueProtocol)
        );
    }
}
