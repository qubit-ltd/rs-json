// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Regression tests for private serde_json value adapters.

use qubit_budget::json::JsonEncodeLimits;
use qubit_budget::json::JsonEncodeSession;
use qubit_json::text::encode_to_vec;
use serde::Serialize;
use serde::Serializer;
use serde::ser::SerializeMap;
use serde::ser::SerializeSeq;
use serde::ser::SerializeStruct;
use serde::ser::SerializeStructVariant;
use serde::ser::SerializeTuple;
use serde::ser::SerializeTupleStruct;
use serde::ser::SerializeTupleVariant;
use serde_json::Number;
use serde_json::from_str;

/// Verifies arbitrary-precision numbers use the number-byte budget.
#[test]
fn test_budgeted_private_value_checks_number_bytes() {
    let number: Number = from_str("123456789").expect("number should parse");
    let limits = JsonEncodeLimits::empty().with_max_number_bytes(8);
    let mut session = JsonEncodeSession::owned(limits);

    assert!(encode_to_vec(&number, &mut session).is_err());
}

#[derive(Clone, Copy)]
enum PrivateScalar {
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
    Unit,
    Some,
    UnitStruct,
    UnitVariant,
    Newtype,
    NewtypeVariant,
    Seq,
    Tuple,
    TupleStruct,
    TupleVariant,
    Map,
    Struct,
    StructVariant,
    HumanReadable,
}

impl Serialize for PrivateScalar {
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
            Self::Bytes => serializer.serialize_bytes(b"bytes"),
            Self::None => serializer.serialize_none(),
            Self::Unit => serializer.serialize_unit(),
            Self::Some => serializer.serialize_some(&"text"),
            Self::UnitStruct => serializer.serialize_unit_struct("Value"),
            Self::UnitVariant => {
                serializer.serialize_unit_variant("Value", 0, "Unit")
            }
            Self::Newtype => {
                serializer.serialize_newtype_struct("Value", &"text")
            }
            Self::NewtypeVariant => serializer
                .serialize_newtype_variant("Value", 0, "Variant", &"text"),
            Self::Seq => {
                serializer.serialize_seq(Some(0)).and_then(|seq| seq.end())
            }
            Self::Tuple => {
                serializer.serialize_tuple(0).and_then(|tuple| tuple.end())
            }
            Self::TupleStruct => serializer
                .serialize_tuple_struct("Value", 0)
                .and_then(|tuple| tuple.end()),
            Self::TupleVariant => serializer
                .serialize_tuple_variant("Value", 0, "Tuple", 0)
                .and_then(|tuple| tuple.end()),
            Self::Map => {
                serializer.serialize_map(Some(0)).and_then(|map| map.end())
            }
            Self::Struct => serializer
                .serialize_struct("Value", 0)
                .and_then(|value| value.end()),
            Self::StructVariant => serializer
                .serialize_struct_variant("Value", 0, "Struct", 0)
                .and_then(|value| value.end()),
            Self::HumanReadable => {
                let _ = serializer.is_human_readable();
                serializer.serialize_str("text")
            }
        }
    }
}

struct PrivateShape {
    raw: bool,
    value: PrivateScalar,
}

impl Serialize for PrivateShape {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let name = if self.raw {
            concat!("$", "serde_json", ":", ":private::RawValue")
        } else {
            concat!("$", "serde_json", ":", ":private::Number")
        };
        let mut value = serializer.serialize_struct(name, 1)?;
        value.serialize_field(name, &self.value)?;
        value.end()
    }
}

/// Drives the private serde_json struct adapter through every delegated
/// serializer method, while preserving the public encoding entry point.
#[test]
fn test_budgeted_private_value_delegates_scalar_serializer_paths() {
    let values = [
        PrivateScalar::Bool,
        PrivateScalar::I8,
        PrivateScalar::I16,
        PrivateScalar::I32,
        PrivateScalar::I64,
        PrivateScalar::I128,
        PrivateScalar::U8,
        PrivateScalar::U16,
        PrivateScalar::U32,
        PrivateScalar::U64,
        PrivateScalar::U128,
        PrivateScalar::F32,
        PrivateScalar::F64,
        PrivateScalar::Char,
        PrivateScalar::Bytes,
        PrivateScalar::None,
        PrivateScalar::Unit,
        PrivateScalar::Some,
        PrivateScalar::UnitStruct,
        PrivateScalar::UnitVariant,
        PrivateScalar::Newtype,
        PrivateScalar::NewtypeVariant,
        PrivateScalar::Seq,
        PrivateScalar::Tuple,
        PrivateScalar::TupleStruct,
        PrivateScalar::TupleVariant,
        PrivateScalar::Map,
        PrivateScalar::Struct,
        PrivateScalar::StructVariant,
        PrivateScalar::HumanReadable,
    ];
    for value in values {
        for raw in [false, true] {
            let mut session =
                JsonEncodeSession::owned(JsonEncodeLimits::empty());
            let _ = encode_to_vec(&PrivateShape { raw, value }, &mut session);
        }
    }
}
