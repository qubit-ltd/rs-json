// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Serde serializer for strict JSON object keys.

use std::fmt::Display;
use std::fmt::Write as _;
use std::str::FromStr;

use serde::Serialize;
use serde::Serializer;
use serde::ser::Impossible;
use serde_json::Number;

use crate::encode::JsonMapKeyKind;
use crate::encode::JsonSerializationError;
use crate::encode::JsonSerializationErrorKind;
use crate::internal::JsonMapKey;

/// Converts supported scalar map keys into JSON object key strings.
#[derive(Debug, Clone, Copy)]
pub(in crate::value::json_value_encoder) struct JsonValueMapKeySerializer;

/// Implements canonical textual serialization for integer key types.
macro_rules! serialize_key_integer {
    ($($method:ident($type:ty)),+ $(,)?) => {
        $(
            #[doc = "Serializes an integer key as canonical decimal text."]
            fn $method(self, value: $type) -> Result<String, Self::Error> {
                Ok(value.to_string())
            }
        )+
    };
}

impl Serializer for JsonValueMapKeySerializer {
    type Ok = String;
    type Error = JsonSerializationError;
    type SerializeSeq = Impossible<String, JsonSerializationError>;
    type SerializeTuple = Impossible<String, JsonSerializationError>;
    type SerializeTupleStruct = Impossible<String, JsonSerializationError>;
    type SerializeTupleVariant = Impossible<String, JsonSerializationError>;
    type SerializeMap = Impossible<String, JsonSerializationError>;
    type SerializeStruct = Impossible<String, JsonSerializationError>;
    type SerializeStructVariant = Impossible<String, JsonSerializationError>;

    /// Serializes a Boolean key through its JSON text.
    fn serialize_bool(self, value: bool) -> Result<String, Self::Error> {
        Ok(value.to_string())
    }

    serialize_key_integer!(
        serialize_i8(i8),
        serialize_i16(i16),
        serialize_i32(i32),
        serialize_i64(i64),
        serialize_u8(u8),
        serialize_u16(u16),
        serialize_u32(u32),
        serialize_u64(u64),
    );

    /// Serializes a full-range signed integer key as canonical decimal text.
    fn serialize_i128(self, value: i128) -> Result<String, Self::Error> {
        Ok(JsonMapKey::signed_wide(value))
    }

    /// Serializes a full-range unsigned integer key as canonical decimal text.
    fn serialize_u128(self, value: u128) -> Result<String, Self::Error> {
        Ok(JsonMapKey::unsigned_wide(value))
    }

    /// Serializes a finite 32-bit floating-point key.
    fn serialize_f32(self, value: f32) -> Result<String, Self::Error> {
        if !value.is_finite() {
            return Err(JsonSerializationError::new(JsonSerializationErrorKind::NonFiniteFloat));
        }
        Number::from_str(&value.to_string())
            .map(|number| number.to_string())
            .map_err(|_| JsonSerializationError::new(JsonSerializationErrorKind::InvalidNumberRepresentation))
    }

    /// Serializes a finite 64-bit floating-point key.
    fn serialize_f64(self, value: f64) -> Result<String, Self::Error> {
        Number::from_f64(value)
            .map(|number| number.to_string())
            .ok_or_else(|| JsonSerializationError::new(JsonSerializationErrorKind::NonFiniteFloat))
    }

    /// Serializes a character key.
    #[inline]
    fn serialize_char(self, value: char) -> Result<String, Self::Error> {
        Ok(value.to_string())
    }

    /// Copies a string key.
    #[inline]
    fn serialize_str(self, value: &str) -> Result<String, Self::Error> {
        Ok(value.to_owned())
    }

    /// Rejects byte sequences as object keys.
    fn serialize_bytes(self, _value: &[u8]) -> Result<String, Self::Error> {
        Err(unsupported_key(JsonMapKeyKind::Bytes))
    }

    /// Rejects absent optional keys.
    fn serialize_none(self) -> Result<String, Self::Error> {
        Err(unsupported_key(JsonMapKeyKind::None))
    }

    /// Delegates a present optional key to its wrapped value.
    fn serialize_some<T>(self, value: &T) -> Result<String, Self::Error>
    where
        T: Serialize + ?Sized,
    {
        value.serialize(self)
    }

    /// Rejects unit keys.
    fn serialize_unit(self) -> Result<String, Self::Error> {
        Err(unsupported_key(JsonMapKeyKind::Unit))
    }

    /// Rejects unit-struct keys.
    fn serialize_unit_struct(self, _name: &'static str) -> Result<String, Self::Error> {
        Err(unsupported_key(JsonMapKeyKind::UnitStruct))
    }

    /// Serializes a unit variant through its variant name.
    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<String, Self::Error> {
        Ok(variant.to_owned())
    }

    /// Delegates a newtype-struct key to its wrapped value.
    fn serialize_newtype_struct<T>(self, _name: &'static str, value: &T) -> Result<String, Self::Error>
    where
        T: Serialize + ?Sized,
    {
        value.serialize(self)
    }

    /// Rejects newtype-variant keys.
    fn serialize_newtype_variant<T>(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _value: &T,
    ) -> Result<String, Self::Error>
    where
        T: Serialize + ?Sized,
    {
        Err(unsupported_key(JsonMapKeyKind::NewtypeVariant))
    }

    /// Rejects sequence keys.
    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        Err(unsupported_key(JsonMapKeyKind::Sequence))
    }

    /// Rejects tuple keys.
    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        Err(unsupported_key(JsonMapKeyKind::Tuple))
    }

    /// Rejects tuple-struct keys.
    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        Err(unsupported_key(JsonMapKeyKind::TupleStruct))
    }

    /// Rejects tuple-variant keys.
    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        Err(unsupported_key(JsonMapKeyKind::TupleVariant))
    }

    /// Rejects map keys.
    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        Err(unsupported_key(JsonMapKeyKind::Map))
    }

    /// Rejects struct keys.
    fn serialize_struct(self, _name: &'static str, _len: usize) -> Result<Self::SerializeStruct, Self::Error> {
        Err(unsupported_key(JsonMapKeyKind::Struct))
    }

    /// Rejects struct-variant keys.
    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        Err(unsupported_key(JsonMapKeyKind::StructVariant))
    }

    /// Serializes a displayable key through its textual representation.
    fn collect_str<T>(self, value: &T) -> Result<String, Self::Error>
    where
        T: Display + ?Sized,
    {
        let mut text = String::new();
        write!(&mut text, "{value}")
            .map_err(|_| JsonSerializationError::new(JsonSerializationErrorKind::DisplayFormattingFailed))?;
        Ok(text)
    }
}

/// Creates a stable unsupported-key failure without retaining key data.
#[inline(always)]
fn unsupported_key(kind: JsonMapKeyKind) -> JsonSerializationError {
    JsonSerializationError::new(JsonSerializationErrorKind::UnsupportedMapKey { kind })
}
