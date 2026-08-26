// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Root Serde serializer for strict materialized JSON values.

use std::fmt::Display;
use std::str::FromStr;

use serde::Serialize;
use serde::Serializer;
use serde_json::Map;
use serde_json::Number;
use serde_json::Value;

use super::JsonValueCompound;
use crate::decode::JsonDecoder;
use crate::value::DuplicateKeyRejectingJsonValue;
use crate::value::JsonValueEncodeError;

/// Maximum compound capacity reserved from an untrusted Serde length hint.
const MAX_PREALLOCATED_ITEMS: usize = 1_024;

/// Serde RawValue protocol token documented by serde_json's public behavior.
pub(in crate::value::json_value_encoder) const RAW_VALUE_TOKEN: &str =
    concat!("$", "serde_json", ":", ":private::RawValue");

/// Bounds a Serde compound length hint before allocating its initial storage.
#[inline(always)]
#[must_use]
fn preallocated_capacity(len: Option<usize>) -> usize {
    len.unwrap_or(0).min(MAX_PREALLOCATED_ITEMS)
}

/// Strictly admits and materializes one RawValue text payload.
pub(in crate::value::json_value_encoder) fn decode_raw_value(text: &str) -> Result<Value, JsonValueEncodeError> {
    let value = JsonDecoder::unlimited()
        .decode_str::<DuplicateKeyRejectingJsonValue>(text)
        .map_err(|_| JsonValueEncodeError::Serialization)?;
    Ok(value.into_inner())
}

/// Builds one strict [`Value`] from Serde events.
#[derive(Debug, Clone, Copy)]
pub(in crate::value::json_value_encoder) struct JsonValueSerializer;

impl Serializer for JsonValueSerializer {
    type Ok = Value;
    type Error = JsonValueEncodeError;
    type SerializeSeq = JsonValueCompound;
    type SerializeTuple = JsonValueCompound;
    type SerializeTupleStruct = JsonValueCompound;
    type SerializeTupleVariant = JsonValueCompound;
    type SerializeMap = JsonValueCompound;
    type SerializeStruct = JsonValueCompound;
    type SerializeStructVariant = JsonValueCompound;

    /// Serializes a Boolean JSON value.
    #[inline(always)]
    fn serialize_bool(self, value: bool) -> Result<Value, Self::Error> {
        Ok(Value::Bool(value))
    }

    /// Serializes an 8-bit signed integer.
    #[inline(always)]
    fn serialize_i8(self, value: i8) -> Result<Value, Self::Error> {
        self.serialize_i64(value.into())
    }

    /// Serializes a 16-bit signed integer.
    #[inline(always)]
    fn serialize_i16(self, value: i16) -> Result<Value, Self::Error> {
        self.serialize_i64(value.into())
    }

    /// Serializes a 32-bit signed integer.
    #[inline(always)]
    fn serialize_i32(self, value: i32) -> Result<Value, Self::Error> {
        self.serialize_i64(value.into())
    }

    /// Serializes a JSON-compatible signed integer.
    #[inline(always)]
    fn serialize_i64(self, value: i64) -> Result<Value, Self::Error> {
        Ok(Value::Number(value.into()))
    }

    /// Serializes a signed wide integer within the strict 64-bit range.
    fn serialize_i128(self, value: i128) -> Result<Value, Self::Error> {
        if let Ok(value) = i64::try_from(value) {
            self.serialize_i64(value)
        } else if let Ok(value) = u64::try_from(value) {
            self.serialize_u64(value)
        } else {
            Err(JsonValueEncodeError::Serialization)
        }
    }

    /// Serializes an 8-bit unsigned integer.
    #[inline(always)]
    fn serialize_u8(self, value: u8) -> Result<Value, Self::Error> {
        self.serialize_u64(value.into())
    }

    /// Serializes a 16-bit unsigned integer.
    #[inline(always)]
    fn serialize_u16(self, value: u16) -> Result<Value, Self::Error> {
        self.serialize_u64(value.into())
    }

    /// Serializes a 32-bit unsigned integer.
    #[inline(always)]
    fn serialize_u32(self, value: u32) -> Result<Value, Self::Error> {
        self.serialize_u64(value.into())
    }

    /// Serializes a JSON-compatible unsigned integer.
    #[inline(always)]
    fn serialize_u64(self, value: u64) -> Result<Value, Self::Error> {
        Ok(Value::Number(value.into()))
    }

    /// Serializes an unsigned wide integer within the strict 64-bit range.
    fn serialize_u128(self, value: u128) -> Result<Value, Self::Error> {
        u64::try_from(value)
            .map_err(|_| JsonValueEncodeError::Serialization)
            .and_then(|value| self.serialize_u64(value))
    }

    /// Serializes a finite f32 without widening its display representation.
    fn serialize_f32(self, value: f32) -> Result<Value, Self::Error> {
        if !value.is_finite() {
            return Err(JsonValueEncodeError::NonFiniteFloat);
        }
        Number::from_str(&value.to_string())
            .map(Value::Number)
            .map_err(|_| JsonValueEncodeError::Serialization)
    }

    /// Serializes a finite 64-bit floating-point number.
    fn serialize_f64(self, value: f64) -> Result<Value, Self::Error> {
        Number::from_f64(value)
            .map(Value::Number)
            .ok_or(JsonValueEncodeError::NonFiniteFloat)
    }

    /// Serializes a character as a JSON string.
    #[inline]
    fn serialize_char(self, value: char) -> Result<Value, Self::Error> {
        Ok(Value::String(value.to_string()))
    }

    /// Copies a borrowed string into a JSON string.
    #[inline]
    fn serialize_str(self, value: &str) -> Result<Value, Self::Error> {
        Ok(Value::String(value.to_owned()))
    }

    /// Serializes bytes as an array of unsigned integer values.
    fn serialize_bytes(self, value: &[u8]) -> Result<Value, Self::Error> {
        Ok(Value::Array(
            value.iter().map(|value| Value::Number((*value).into())).collect(),
        ))
    }

    /// Serializes an absent option as JSON null.
    #[inline(always)]
    fn serialize_none(self) -> Result<Value, Self::Error> {
        self.serialize_unit()
    }

    /// Serializes the value inside a present option.
    #[inline(always)]
    fn serialize_some<T>(self, value: &T) -> Result<Value, Self::Error>
    where
        T: Serialize + ?Sized,
    {
        value.serialize(self)
    }

    /// Serializes a unit value as JSON null.
    #[inline(always)]
    fn serialize_unit(self) -> Result<Value, Self::Error> {
        Ok(Value::Null)
    }

    /// Serializes a unit struct as JSON null.
    #[inline(always)]
    fn serialize_unit_struct(self, _name: &'static str) -> Result<Value, Self::Error> {
        self.serialize_unit()
    }

    /// Serializes a unit variant through its variant name.
    #[inline(always)]
    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<Value, Self::Error> {
        self.serialize_str(variant)
    }

    /// Delegates ordinary newtype structs and materializes RawValue payloads.
    fn serialize_newtype_struct<T>(self, name: &'static str, value: &T) -> Result<Value, Self::Error>
    where
        T: Serialize + ?Sized,
    {
        let value = value.serialize(self)?;
        if name != RAW_VALUE_TOKEN {
            return Ok(value);
        }
        let Value::String(text) = value else {
            return Err(JsonValueEncodeError::Serialization);
        };
        decode_raw_value(&text)
    }

    /// Serializes a newtype variant as a single-key object.
    fn serialize_newtype_variant<T>(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<Value, Self::Error>
    where
        T: Serialize + ?Sized,
    {
        let mut object = Map::new();
        object.insert(variant.to_owned(), value.serialize(self)?);
        Ok(Value::Object(object))
    }

    /// Creates a sequence serializer with a bounded initial capacity.
    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        Ok(JsonValueCompound::sequence(preallocated_capacity(len)))
    }

    /// Creates a tuple serializer with a bounded initial capacity.
    #[inline(always)]
    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        self.serialize_seq(Some(len))
    }

    /// Creates a tuple-struct serializer with a bounded initial capacity.
    #[inline(always)]
    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        self.serialize_seq(Some(len))
    }

    /// Creates a tuple-variant serializer with a bounded initial capacity.
    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        Ok(JsonValueCompound::tuple_variant(
            variant,
            preallocated_capacity(Some(len)),
        ))
    }

    /// Creates a map serializer with a bounded initial capacity.
    fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        Ok(JsonValueCompound::map(preallocated_capacity(len)))
    }

    /// Creates a struct serializer with a bounded initial capacity.
    fn serialize_struct(self, name: &'static str, len: usize) -> Result<Self::SerializeStruct, Self::Error> {
        if name == RAW_VALUE_TOKEN {
            Ok(JsonValueCompound::raw_value())
        } else {
            Ok(JsonValueCompound::map(preallocated_capacity(Some(len))))
        }
    }

    /// Creates a struct-variant serializer with bounded initial capacity.
    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        Ok(JsonValueCompound::struct_variant(
            variant,
            preallocated_capacity(Some(len)),
        ))
    }

    /// Serializes a displayable value as a JSON string.
    fn collect_str<T>(self, value: &T) -> Result<Value, Self::Error>
    where
        T: Display + ?Sized,
    {
        self.serialize_str(&value.to_string())
    }
}
