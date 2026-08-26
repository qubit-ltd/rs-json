// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Serde serializer for strict JSON object keys.

use std::fmt::Display;
use std::str::FromStr;

use serde::Serialize;
use serde::Serializer;
use serde::ser::Impossible;
use serde_json::Number;

use crate::value::JsonValueEncodeError;

/// Converts supported scalar map keys into JSON object key strings.
#[derive(Debug, Clone, Copy)]
pub(in crate::value::json_value_encoder) struct JsonValueMapKeySerializer;

/// Implements canonical textual serialization for integer key types.
macro_rules! serialize_key_integer {
    ($($method:ident($type:ty)),+ $(,)?) => {
        $(
            fn $method(self, value: $type) -> Result<String, Self::Error> {
                Ok(value.to_string())
            }
        )+
    };
}

impl Serializer for JsonValueMapKeySerializer {
    type Ok = String;
    type Error = JsonValueEncodeError;
    type SerializeSeq = Impossible<String, JsonValueEncodeError>;
    type SerializeTuple = Impossible<String, JsonValueEncodeError>;
    type SerializeTupleStruct = Impossible<String, JsonValueEncodeError>;
    type SerializeTupleVariant = Impossible<String, JsonValueEncodeError>;
    type SerializeMap = Impossible<String, JsonValueEncodeError>;
    type SerializeStruct = Impossible<String, JsonValueEncodeError>;
    type SerializeStructVariant = Impossible<String, JsonValueEncodeError>;

    /// Serializes a Boolean key through its JSON text.
    fn serialize_bool(self, value: bool) -> Result<String, Self::Error> {
        Ok(value.to_string())
    }

    serialize_key_integer!(
        serialize_i8(i8),
        serialize_i16(i16),
        serialize_i32(i32),
        serialize_i64(i64),
        serialize_i128(i128),
        serialize_u8(u8),
        serialize_u16(u16),
        serialize_u32(u32),
        serialize_u64(u64),
        serialize_u128(u128),
    );

    /// Serializes a finite 32-bit floating-point key.
    fn serialize_f32(self, value: f32) -> Result<String, Self::Error> {
        if !value.is_finite() {
            return Err(JsonValueEncodeError::NonFiniteFloat);
        }
        Number::from_str(&value.to_string())
            .map(|number| number.to_string())
            .map_err(|_| JsonValueEncodeError::Serialization)
    }

    /// Serializes a finite 64-bit floating-point key.
    fn serialize_f64(self, value: f64) -> Result<String, Self::Error> {
        Number::from_f64(value)
            .map(|number| number.to_string())
            .ok_or(JsonValueEncodeError::NonFiniteFloat)
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
        Err(JsonValueEncodeError::Serialization)
    }

    /// Rejects absent optional keys.
    fn serialize_none(self) -> Result<String, Self::Error> {
        Err(JsonValueEncodeError::Serialization)
    }

    /// Rejects optional key wrappers.
    fn serialize_some<T>(self, _value: &T) -> Result<String, Self::Error>
    where
        T: Serialize + ?Sized,
    {
        Err(JsonValueEncodeError::Serialization)
    }

    /// Rejects unit keys.
    fn serialize_unit(self) -> Result<String, Self::Error> {
        Err(JsonValueEncodeError::Serialization)
    }

    /// Rejects unit-struct keys.
    fn serialize_unit_struct(self, _name: &'static str) -> Result<String, Self::Error> {
        Err(JsonValueEncodeError::Serialization)
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
        Err(JsonValueEncodeError::Serialization)
    }

    /// Rejects sequence keys.
    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        Err(JsonValueEncodeError::Serialization)
    }

    /// Rejects tuple keys.
    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        Err(JsonValueEncodeError::Serialization)
    }

    /// Rejects tuple-struct keys.
    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        Err(JsonValueEncodeError::Serialization)
    }

    /// Rejects tuple-variant keys.
    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        Err(JsonValueEncodeError::Serialization)
    }

    /// Rejects map keys.
    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        Err(JsonValueEncodeError::Serialization)
    }

    /// Rejects struct keys.
    fn serialize_struct(self, _name: &'static str, _len: usize) -> Result<Self::SerializeStruct, Self::Error> {
        Err(JsonValueEncodeError::Serialization)
    }

    /// Rejects struct-variant keys.
    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        Err(JsonValueEncodeError::Serialization)
    }

    /// Serializes a displayable key through its textual representation.
    fn collect_str<T>(self, value: &T) -> Result<String, Self::Error>
    where
        T: Display + ?Sized,
    {
        Ok(value.to_string())
    }
}
