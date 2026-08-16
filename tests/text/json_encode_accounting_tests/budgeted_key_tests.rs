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
use serde::Serialize;
use serde::Serializer;
use serde::ser::SerializeMap;
use serde::ser::SerializeSeq;
use serde::ser::SerializeStruct;
use serde::ser::SerializeStructVariant;
use serde::ser::SerializeTuple;
use serde::ser::SerializeTupleStruct;
use serde::ser::SerializeTupleVariant;

use crate::text::json_encode_test_support::encode;

/// Verifies object keys consume the key-byte budget.
#[test]
fn test_budgeted_key_checks_key_bytes() {
    let values = BTreeMap::from([(String::from("key"), true)]);
    let limits = JsonEncodeLimits::<JsonResource, usize>::builder()
        .max_key_bytes(2)
        .build();
    let mut session = JsonEncodeSession::owned(limits);

    assert!(encode(&values, &mut session).is_err());
}

#[derive(Clone, Copy)]
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
            Self::Char => serializer.serialize_char('x'),
            Self::Str => serializer.serialize_str("key"),
            Self::Bytes => serializer.serialize_bytes(b"key"),
            Self::None => serializer.serialize_none(),
            Self::Unit => serializer.serialize_unit(),
            Self::UnitStruct => serializer.serialize_unit_struct("Key"),
            Self::UnitVariant => {
                serializer.serialize_unit_variant("Key", 0, "Unit")
            }
            Self::NewtypeStruct => {
                serializer.serialize_newtype_struct("Key", &"value")
            }
            Self::NewtypeVariant => serializer
                .serialize_newtype_variant("Key", 0, "Variant", &"value"),
            Self::Some => serializer.serialize_some(&"value"),
            Self::Seq => {
                serializer.serialize_seq(Some(0)).and_then(|seq| seq.end())
            }
            Self::Tuple => {
                serializer.serialize_tuple(0).and_then(|tuple| tuple.end())
            }
            Self::TupleStruct => serializer
                .serialize_tuple_struct("Key", 0)
                .and_then(|tuple| tuple.end()),
            Self::TupleVariant => serializer
                .serialize_tuple_variant("Key", 0, "Tuple", 0)
                .and_then(|tuple| tuple.end()),
            Self::Map => {
                serializer.serialize_map(Some(0)).and_then(|map| map.end())
            }
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
    let keys = [
        ScalarKey::Bool,
        ScalarKey::I8,
        ScalarKey::I16,
        ScalarKey::I32,
        ScalarKey::I64,
        ScalarKey::I128,
        ScalarKey::U8,
        ScalarKey::U16,
        ScalarKey::U32,
        ScalarKey::U64,
        ScalarKey::U128,
        ScalarKey::F32,
        ScalarKey::F64,
        ScalarKey::Char,
        ScalarKey::Str,
        ScalarKey::Bytes,
        ScalarKey::None,
        ScalarKey::Unit,
        ScalarKey::UnitStruct,
        ScalarKey::UnitVariant,
        ScalarKey::NewtypeStruct,
        ScalarKey::NewtypeVariant,
        ScalarKey::Some,
        ScalarKey::Seq,
        ScalarKey::Tuple,
        ScalarKey::TupleStruct,
        ScalarKey::TupleVariant,
        ScalarKey::Map,
        ScalarKey::Struct,
        ScalarKey::StructVariant,
        ScalarKey::HumanReadable,
    ];
    for key in keys {
        let mut session = JsonEncodeSession::owned(
            JsonEncodeLimits::<JsonResource, usize>::builder().build(),
        );
        let _ = encode(&OneKey(key), &mut session);
    }
}
