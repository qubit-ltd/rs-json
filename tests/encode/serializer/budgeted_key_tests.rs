// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests budgeted JSON object keys.

use std::collections::BTreeMap;

use qubit_budget::json::JsonEncodeLimits;
use qubit_budget::json::JsonEncodeSession;
use qubit_budget::json::JsonResource;
use qubit_json::encode::JsonMapKeyKind;
use qubit_json::encode::JsonSerializationErrorKind;
use serde::Serialize;
use serde::Serializer;
use serde::ser::SerializeMap;
use serde::ser::SerializeSeq;
use serde::ser::SerializeStruct;
use serde::ser::SerializeStructVariant;
use serde::ser::SerializeTuple;
use serde::ser::SerializeTupleStruct;
use serde::ser::SerializeTupleVariant;

use crate::encode::json_encode_test_support::encode;

/// Verifies object keys consume the key-byte budget.
#[test]
fn test_budgeted_key_checks_key_bytes() {
    let values = BTreeMap::from([(String::from("key"), true)]);
    let limits = JsonEncodeLimits::<JsonResource, usize>::builder()
        .max_key_bytes(2)
        .build();
    let mut session = JsonEncodeSession::from_limits(limits);

    assert!(encode(&values, &mut session).is_err());
}

#[derive(Debug, Clone, Copy)]
enum ScalarKey {
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
    F64NonFinite,
    Char,
    Str,
    Bytes,
    None,
    Unit,
    UnitStruct,
    UnitVariant,
    NewtypeStruct,
    NewtypeVariant,
    Some,
    Seq,
    Tuple,
    TupleStruct,
    TupleVariant,
    Map,
    Struct,
    StructVariant,
    HumanReadable,
}

impl Serialize for ScalarKey {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::Bool => serializer.serialize_bool(true),
            Self::I8 => serializer.serialize_i8(-8),
            Self::I16 => serializer.serialize_i16(-16),
            Self::I32 => serializer.serialize_i32(-32),
            Self::I64 => serializer.serialize_i64(-64),
            Self::I128 => serializer.serialize_i128(-128),
            Self::U8 => serializer.serialize_u8(8),
            Self::U16 => serializer.serialize_u16(16),
            Self::U32 => serializer.serialize_u32(32),
            Self::U64 => serializer.serialize_u64(64),
            Self::U128 => serializer.serialize_u128(128),
            Self::F32 => serializer.serialize_f32(1.25),
            Self::F64 => serializer.serialize_f64(2.5),
            Self::F64NonFinite => serializer.serialize_f64(f64::INFINITY),
            Self::Char => serializer.serialize_char('x'),
            Self::Str => serializer.serialize_str("key"),
            Self::Bytes => serializer.serialize_bytes(b"key"),
            Self::None => serializer.serialize_none(),
            Self::Unit => serializer.serialize_unit(),
            Self::UnitStruct => serializer.serialize_unit_struct("Key"),
            Self::UnitVariant => serializer.serialize_unit_variant("Key", 0, "Unit"),
            Self::NewtypeStruct => serializer.serialize_newtype_struct("Key", &"value"),
            Self::NewtypeVariant => {
                serializer.serialize_newtype_variant("Key", 0, "Variant", &"value")
            }
            Self::Some => serializer.serialize_some(&"value"),
            Self::Seq => serializer.serialize_seq(Some(0)).and_then(|seq| seq.end()),
            Self::Tuple => serializer.serialize_tuple(0).and_then(|tuple| tuple.end()),
            Self::TupleStruct => serializer
                .serialize_tuple_struct("Key", 0)
                .and_then(|tuple| tuple.end()),
            Self::TupleVariant => serializer
                .serialize_tuple_variant("Key", 0, "Tuple", 0)
                .and_then(|tuple| tuple.end()),
            Self::Map => serializer.serialize_map(Some(0)).and_then(|map| map.end()),
            Self::Struct => serializer
                .serialize_struct("Key", 0)
                .and_then(|value| value.end()),
            Self::StructVariant => serializer
                .serialize_struct_variant("Key", 0, "Struct", 0)
                .and_then(|value| value.end()),
            Self::HumanReadable => {
                let _ = serializer.is_human_readable();
                serializer.serialize_str("human")
            }
        }
    }
}

struct OneKey(ScalarKey);

impl Serialize for OneKey {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(1))?;
        map.serialize_entry(&self.0, &true)?;
        map.end()
    }
}

/// Exercises all scalar/key serializer dispatch paths through the public
/// budgeted JSON encoder.
#[test]
fn test_budgeted_key_checks_each_scalar_serializer_path() {
    enum Expected {
        Bytes(&'static [u8]),
        KeyError(JsonMapKeyKind),
        Error(JsonSerializationErrorKind),
    }
    let cases = [
        (ScalarKey::Bool, Expected::Bytes(br#"{"true":true}"#)),
        (ScalarKey::I8, Expected::Bytes(br#"{"-8":true}"#)),
        (ScalarKey::I16, Expected::Bytes(br#"{"-16":true}"#)),
        (ScalarKey::I32, Expected::Bytes(br#"{"-32":true}"#)),
        (ScalarKey::I64, Expected::Bytes(br#"{"-64":true}"#)),
        (ScalarKey::I128, Expected::Bytes(br#"{"-128":true}"#)),
        (ScalarKey::U8, Expected::Bytes(br#"{"8":true}"#)),
        (ScalarKey::U16, Expected::Bytes(br#"{"16":true}"#)),
        (ScalarKey::U32, Expected::Bytes(br#"{"32":true}"#)),
        (ScalarKey::U64, Expected::Bytes(br#"{"64":true}"#)),
        (ScalarKey::U128, Expected::Bytes(br#"{"128":true}"#)),
        (ScalarKey::F32, Expected::Bytes(br#"{"1.25":true}"#)),
        (ScalarKey::F64, Expected::Bytes(br#"{"2.5":true}"#)),
        (
            ScalarKey::F64NonFinite,
            Expected::Error(JsonSerializationErrorKind::NonFiniteFloat),
        ),
        (ScalarKey::Char, Expected::Bytes(br#"{"x":true}"#)),
        (ScalarKey::Str, Expected::Bytes(br#"{"key":true}"#)),
        (ScalarKey::Bytes, Expected::KeyError(JsonMapKeyKind::Bytes)),
        (ScalarKey::None, Expected::KeyError(JsonMapKeyKind::None)),
        (ScalarKey::Unit, Expected::KeyError(JsonMapKeyKind::Unit)),
        (
            ScalarKey::UnitStruct,
            Expected::KeyError(JsonMapKeyKind::UnitStruct),
        ),
        (ScalarKey::UnitVariant, Expected::Bytes(br#"{"Unit":true}"#)),
        (
            ScalarKey::NewtypeStruct,
            Expected::Bytes(br#"{"value":true}"#),
        ),
        (
            ScalarKey::NewtypeVariant,
            Expected::KeyError(JsonMapKeyKind::NewtypeVariant),
        ),
        (ScalarKey::Some, Expected::Bytes(br#"{"value":true}"#)),
        (ScalarKey::Seq, Expected::KeyError(JsonMapKeyKind::Sequence)),
        (ScalarKey::Tuple, Expected::KeyError(JsonMapKeyKind::Tuple)),
        (
            ScalarKey::TupleStruct,
            Expected::KeyError(JsonMapKeyKind::TupleStruct),
        ),
        (
            ScalarKey::TupleVariant,
            Expected::KeyError(JsonMapKeyKind::TupleVariant),
        ),
        (ScalarKey::Map, Expected::KeyError(JsonMapKeyKind::Map)),
        (
            ScalarKey::Struct,
            Expected::KeyError(JsonMapKeyKind::Struct),
        ),
        (
            ScalarKey::StructVariant,
            Expected::KeyError(JsonMapKeyKind::StructVariant),
        ),
        (
            ScalarKey::HumanReadable,
            Expected::Bytes(br#"{"human":true}"#),
        ),
    ];
    for (key, expected) in cases {
        let mut session = JsonEncodeSession::from_limits(
            JsonEncodeLimits::<JsonResource, usize>::builder().build(),
        );
        let result = encode(&OneKey(key), &mut session);
        match expected {
            Expected::Bytes(bytes) => assert_eq!(
                result.expect("supported scalar key must encode"),
                bytes,
                "case={key:?}"
            ),
            Expected::KeyError(kind)
            | Expected::Error(JsonSerializationErrorKind::UnsupportedMapKey { kind }) => {
                assert_eq!(
                    result
                        .expect_err("unsupported scalar key must be rejected")
                        .serialization_error()
                        .expect("key rejection is a serialization error")
                        .kind(),
                    JsonSerializationErrorKind::UnsupportedMapKey { kind },
                    "case={key:?}",
                )
            }
            Expected::Error(kind) => assert_eq!(
                result
                    .expect_err("invalid scalar key must be rejected")
                    .serialization_error()
                    .expect("key rejection is a serialization error")
                    .kind(),
                kind,
                "case={key:?}",
            ),
        }
    }
}
