// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Root Serde serializer for strict materialized JSON values.

use std::fmt::Display;
use std::fmt::Write as _;

use qubit_budget::json::JsonResource;
use qubit_budget::json::JsonValueBudget;
use qubit_budget::json::JsonValueLimits;
use serde::Serialize;
use serde::Serializer;
use serde_json::Map;
use serde_json::Number;
use serde_json::Value;
use serde_json::to_string;

use super::JsonValueCompound;
use crate::encode::JsonIntegerSignedness;
use crate::encode::JsonSerializationError;
use crate::encode::JsonSerializationErrorKind;
use crate::internal::JsonPrivateProtocolKind;
use crate::internal::JsonPrivateProtocolState;
use crate::lexical::JsonLexicalScanner;
use crate::lexical::JsonNumberModel;
use crate::value::internal::JsonValueMaterializationError;
use crate::value::internal::LexicalJsonValueBuilder;

/// Maximum compound capacity reserved from an untrusted Serde length hint.
const MAX_PREALLOCATED_ITEMS: usize = 1_024;

/// Bounds a Serde compound length hint before allocating its initial storage.
///
/// # Parameters
///
/// * `len` - Optional size hint supplied by the Serde data source.
///
/// # Returns
///
/// The hint clamped to [`MAX_PREALLOCATED_ITEMS`], or zero when no hint exists.
#[inline(always)]
#[must_use]
fn preallocated_capacity(len: Option<usize>) -> usize {
    len.unwrap_or(0).min(MAX_PREALLOCATED_ITEMS)
}

/// Strictly admits and materializes one RawValue text payload.
///
/// # Errors
///
/// Returns [`JsonSerializationErrorKind::InvalidRawValue`] when the text is
/// invalid JSON, contains duplicate object keys, or exceeds the fixed maximum
/// of 127 nested containers (container depths 1 through 127 are accepted).
///
/// # Parameters
///
/// * `text` - Raw JSON text emitted by Serde's `RawValue` protocol.
///
/// # Returns
///
/// The materialized JSON value when the payload passes strict validation.
pub(in crate::value::json_value_encoder) fn decode_raw_value(text: &str) -> Result<Value, JsonSerializationError> {
    let limits = JsonValueLimits::<JsonResource, usize>::new();
    let mut budget = JsonValueBudget::new(limits);
    {
        let mut transaction = budget.transaction();
        JsonLexicalScanner::new(&mut transaction, false)
            .with_number_model(JsonNumberModel::Integer64)
            .scan(text.as_bytes())
            .map_err(|_| JsonSerializationError::new(JsonSerializationErrorKind::InvalidRawValue))?;
    }
    let mut builder = LexicalJsonValueBuilder::new(JsonNumberModel::Integer64);
    let mut transaction = budget.transaction();
    JsonLexicalScanner::new(&mut transaction, false)
        .with_number_model(JsonNumberModel::Integer64)
        .scan_with_events(text.as_bytes(), |event| builder.accept(event))
        .map_err(|_| JsonSerializationError::new(JsonSerializationErrorKind::InvalidRawValue))?;
    builder.finish().map_err(|error| {
        match error {
            JsonValueMaterializationError::DuplicateKey { offset }
            | JsonValueMaterializationError::InvalidNumber { offset } => {
                let _ = offset;
            }
            JsonValueMaterializationError::DepthLimit { offset, limit } => {
                let _ = (offset, limit);
            }
        }
        JsonSerializationError::new(JsonSerializationErrorKind::InvalidRawValue)
    })
}

/// Builds one strict [`Value`] from Serde events.
#[derive(Debug, Clone, Copy)]
pub(in crate::value::json_value_encoder) struct JsonValueSerializer;

impl Serializer for JsonValueSerializer {
    type Ok = Value;
    type Error = JsonSerializationError;
    type SerializeSeq = JsonValueCompound;
    type SerializeTuple = JsonValueCompound;
    type SerializeTupleStruct = JsonValueCompound;
    type SerializeTupleVariant = JsonValueCompound;
    type SerializeMap = JsonValueCompound;
    type SerializeStruct = JsonValueCompound;
    type SerializeStructVariant = JsonValueCompound;

    /// Serializes a Boolean JSON value.
    ///
    /// # Parameters
    ///
    /// * `value` - Boolean value to encode.
    ///
    /// # Returns
    ///
    /// The corresponding JSON boolean.
    #[inline(always)]
    fn serialize_bool(self, value: bool) -> Result<Value, Self::Error> {
        Ok(Value::Bool(value))
    }

    /// Serializes an 8-bit signed integer.
    ///
    /// # Parameters
    ///
    /// * `value` - Integer to encode.
    ///
    /// # Returns
    ///
    /// The corresponding JSON number.
    #[inline(always)]
    fn serialize_i8(self, value: i8) -> Result<Value, Self::Error> {
        self.serialize_i64(value.into())
    }

    /// Serializes a 16-bit signed integer.
    ///
    /// # Parameters
    ///
    /// * `value` - Integer to encode.
    ///
    /// # Returns
    ///
    /// The corresponding JSON number.
    #[inline(always)]
    fn serialize_i16(self, value: i16) -> Result<Value, Self::Error> {
        self.serialize_i64(value.into())
    }

    /// Serializes a 32-bit signed integer.
    ///
    /// # Parameters
    ///
    /// * `value` - Integer to encode.
    ///
    /// # Returns
    ///
    /// The corresponding JSON number.
    #[inline(always)]
    fn serialize_i32(self, value: i32) -> Result<Value, Self::Error> {
        self.serialize_i64(value.into())
    }

    /// Serializes a JSON-compatible signed integer.
    ///
    /// # Parameters
    ///
    /// * `value` - Signed integer to encode.
    ///
    /// # Returns
    ///
    /// The corresponding JSON number.
    #[inline(always)]
    fn serialize_i64(self, value: i64) -> Result<Value, Self::Error> {
        Ok(Value::Number(value.into()))
    }

    /// Serializes a signed wide integer within the strict 64-bit range.
    ///
    /// # Errors
    ///
    /// Returns an integer range error when `value` is outside the combined
    /// `i64` and `u64` ranges.
    ///
    /// # Parameters
    ///
    /// * `value` - Signed wide integer to encode.
    ///
    /// # Returns
    ///
    /// The corresponding JSON number when it fits the supported integer range.
    fn serialize_i128(self, value: i128) -> Result<Value, Self::Error> {
        if let Ok(value) = i64::try_from(value) {
            self.serialize_i64(value)
        } else if let Ok(value) = u64::try_from(value) {
            self.serialize_u64(value)
        } else {
            Err(JsonSerializationError::new(
                JsonSerializationErrorKind::IntegerOutOfRange {
                    signedness: JsonIntegerSignedness::Signed,
                },
            ))
        }
    }

    /// Serializes an 8-bit unsigned integer.
    ///
    /// # Parameters
    ///
    /// * `value` - Integer to encode.
    ///
    /// # Returns
    ///
    /// The corresponding JSON number.
    #[inline(always)]
    fn serialize_u8(self, value: u8) -> Result<Value, Self::Error> {
        self.serialize_u64(value.into())
    }

    /// Serializes a 16-bit unsigned integer.
    ///
    /// # Parameters
    ///
    /// * `value` - Integer to encode.
    ///
    /// # Returns
    ///
    /// The corresponding JSON number.
    #[inline(always)]
    fn serialize_u16(self, value: u16) -> Result<Value, Self::Error> {
        self.serialize_u64(value.into())
    }

    /// Serializes a 32-bit unsigned integer.
    ///
    /// # Parameters
    ///
    /// * `value` - Integer to encode.
    ///
    /// # Returns
    ///
    /// The corresponding JSON number.
    #[inline(always)]
    fn serialize_u32(self, value: u32) -> Result<Value, Self::Error> {
        self.serialize_u64(value.into())
    }

    /// Serializes a JSON-compatible unsigned integer.
    ///
    /// # Parameters
    ///
    /// * `value` - Unsigned integer to encode.
    ///
    /// # Returns
    ///
    /// The corresponding JSON number.
    #[inline(always)]
    fn serialize_u64(self, value: u64) -> Result<Value, Self::Error> {
        Ok(Value::Number(value.into()))
    }

    /// Serializes an unsigned wide integer within the strict 64-bit range.
    ///
    /// # Errors
    ///
    /// Returns an integer range error when `value` exceeds `u64::MAX`.
    ///
    /// # Parameters
    ///
    /// * `value` - Unsigned wide integer to encode.
    ///
    /// # Returns
    ///
    /// The corresponding JSON number when `value` fits in `u64`.
    fn serialize_u128(self, value: u128) -> Result<Value, Self::Error> {
        u64::try_from(value)
            .map_err(|_| {
                JsonSerializationError::new(JsonSerializationErrorKind::IntegerOutOfRange {
                    signedness: JsonIntegerSignedness::Unsigned,
                })
            })
            .and_then(|value| self.serialize_u64(value))
    }

    /// Serializes a finite f32 using serde_json's f32 JSON representation.
    ///
    /// # Errors
    ///
    /// Returns a non-finite value error for NaN or infinity, or an invalid
    /// number representation error if serde_json cannot format the value.
    ///
    /// # Parameters
    ///
    /// * `value` - Finite 32-bit floating-point value to encode.
    ///
    /// # Returns
    ///
    /// The JSON number represented by serde_json's f32 formatter.
    fn serialize_f32(self, value: f32) -> Result<Value, Self::Error> {
        if !value.is_finite() {
            return Err(JsonSerializationError::new(JsonSerializationErrorKind::NonFiniteFloat));
        }
        to_string(&value)
            .and_then(|text| serde_json::from_str(&text))
            .map_err(|_| JsonSerializationError::new(JsonSerializationErrorKind::InvalidNumberRepresentation))
    }

    /// Serializes a finite 64-bit floating-point number.
    ///
    /// # Errors
    ///
    /// Returns a non-finite value error for NaN or infinity.
    ///
    /// # Parameters
    ///
    /// * `value` - 64-bit floating-point value to encode.
    ///
    /// # Returns
    ///
    /// The corresponding JSON number for a finite value.
    fn serialize_f64(self, value: f64) -> Result<Value, Self::Error> {
        Number::from_f64(value)
            .map(Value::Number)
            .ok_or_else(|| JsonSerializationError::new(JsonSerializationErrorKind::NonFiniteFloat))
    }

    /// Serializes a character as a JSON string.
    ///
    /// # Parameters
    ///
    /// * `value` - Character to encode.
    ///
    /// # Returns
    ///
    /// A JSON string containing the character.
    #[inline]
    fn serialize_char(self, value: char) -> Result<Value, Self::Error> {
        Ok(Value::String(value.to_string()))
    }

    /// Copies a borrowed string into a JSON string.
    ///
    /// # Parameters
    ///
    /// * `value` - String slice to copy into the JSON value.
    ///
    /// # Returns
    ///
    /// A JSON string containing an owned copy of `value`.
    #[inline]
    fn serialize_str(self, value: &str) -> Result<Value, Self::Error> {
        Ok(Value::String(value.to_owned()))
    }

    /// Serializes bytes as an array of unsigned integer values.
    ///
    /// # Parameters
    ///
    /// * `value` - Byte slice to encode.
    ///
    /// # Returns
    ///
    /// A JSON array containing one unsigned number for each byte.
    fn serialize_bytes(self, value: &[u8]) -> Result<Value, Self::Error> {
        Ok(Value::Array(
            value.iter().map(|value| Value::Number((*value).into())).collect(),
        ))
    }

    /// Serializes an absent option as JSON null.
    ///
    /// # Returns
    ///
    /// JSON null.
    #[inline(always)]
    fn serialize_none(self) -> Result<Value, Self::Error> {
        self.serialize_unit()
    }

    /// Serializes the value inside a present option.
    ///
    /// # Errors
    ///
    /// Returns any error produced while serializing `value`.
    ///
    /// # Type Parameters
    ///
    /// * `T` - Serde-serializable value type, which may be unsized.
    ///
    /// # Parameters
    ///
    /// * `value` - Present option value to serialize.
    ///
    /// # Returns
    ///
    /// The JSON value produced from the contained value.
    #[inline(always)]
    fn serialize_some<T>(self, value: &T) -> Result<Value, Self::Error>
    where
        T: Serialize + ?Sized,
    {
        value.serialize(self)
    }

    /// Serializes a unit value as JSON null.
    ///
    /// # Returns
    ///
    /// JSON null.
    #[inline(always)]
    fn serialize_unit(self) -> Result<Value, Self::Error> {
        Ok(Value::Null)
    }

    /// Serializes a unit struct as JSON null.
    ///
    /// # Parameters
    ///
    /// * `_name` - Serde type name, unused by JSON value encoding.
    ///
    /// # Returns
    ///
    /// JSON null.
    #[inline(always)]
    fn serialize_unit_struct(self, _name: &'static str) -> Result<Value, Self::Error> {
        self.serialize_unit()
    }

    /// Serializes a unit variant through its variant name.
    ///
    /// # Parameters
    ///
    /// * `_name` - Serde enum type name, unused by JSON value encoding.
    /// * `_variant_index` - Serde variant index, unused by JSON value encoding.
    /// * `variant` - Variant name to encode as a JSON string.
    ///
    /// # Returns
    ///
    /// A JSON string containing the variant name.
    #[inline(always)]
    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<Value, Self::Error> {
        self.serialize_str(variant)
    }

    /// Delegates every newtype struct transparently.
    ///
    /// # Errors
    ///
    /// Returns any error produced while serializing `value`.
    ///
    /// # Type Parameters
    ///
    /// * `T` - Serde-serializable value type, which may be unsized.
    ///
    /// # Parameters
    ///
    /// * `_name` - Newtype name, unused because newtype structs are
    ///   transparent.
    /// * `value` - Wrapped value to serialize.
    ///
    /// # Returns
    ///
    /// The JSON value produced from the wrapped value.
    fn serialize_newtype_struct<T>(self, _name: &'static str, value: &T) -> Result<Value, Self::Error>
    where
        T: Serialize + ?Sized,
    {
        value.serialize(self)
    }

    /// Serializes a newtype variant as a single-key object.
    ///
    /// # Errors
    ///
    /// Returns any error produced while serializing `value`.
    ///
    /// # Type Parameters
    ///
    /// * `T` - Serde-serializable value type, which may be unsized.
    ///
    /// # Parameters
    ///
    /// * `_name` - Serde enum type name, unused by JSON value encoding.
    /// * `_variant_index` - Serde variant index, unused by JSON value encoding.
    /// * `variant` - Variant name used as the object's sole key.
    /// * `value` - Wrapped value to serialize.
    ///
    /// # Returns
    ///
    /// A one-key JSON object containing the serialized value.
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
    ///
    /// # Parameters
    ///
    /// * `len` - Optional sequence length hint supplied by Serde.
    ///
    /// # Returns
    ///
    /// A sequence compound with capacity zero when `len` is `None`, or with the
    /// provided capacity bounded by the local limit when `len` is `Some`.
    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        Ok(JsonValueCompound::sequence(preallocated_capacity(len)))
    }

    /// Creates a tuple serializer with a bounded initial capacity.
    ///
    /// # Parameters
    ///
    /// * `len` - Number of tuple elements supplied by Serde.
    ///
    /// # Returns
    ///
    /// A sequence compound with its initial capacity bounded by the local
    /// limit.
    #[inline(always)]
    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        self.serialize_seq(Some(len))
    }

    /// Creates a tuple-struct serializer with a bounded initial capacity.
    ///
    /// # Parameters
    ///
    /// * `_name` - Serde tuple-struct name, unused by JSON value encoding.
    /// * `len` - Number of tuple fields supplied by Serde.
    ///
    /// # Returns
    ///
    /// A sequence compound with its initial capacity bounded by the local
    /// limit.
    #[inline(always)]
    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        self.serialize_seq(Some(len))
    }

    /// Creates a tuple-variant serializer with a bounded initial capacity.
    ///
    /// # Parameters
    ///
    /// * `_name` - Serde enum type name, unused by JSON value encoding.
    /// * `_variant_index` - Serde variant index, unused by JSON value encoding.
    /// * `variant` - Variant name used as the outer object key.
    /// * `len` - Number of tuple fields supplied by Serde.
    ///
    /// # Returns
    ///
    /// A tuple-variant compound with bounded initial field capacity.
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
    ///
    /// # Parameters
    ///
    /// * `len` - Optional map entry count supplied by Serde.
    ///
    /// # Returns
    ///
    /// A map compound with capacity zero when `len` is `None`, or with the
    /// provided capacity bounded by the local limit when `len` is `Some`.
    fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        Ok(JsonValueCompound::map(preallocated_capacity(len)))
    }

    /// Creates a struct serializer with a bounded initial capacity.
    ///
    /// # Errors
    ///
    /// Returns an invalid serializer state error if a recognized private
    /// protocol name has an invalid field count.
    ///
    /// # Parameters
    ///
    /// * `name` - Serde struct name, used to recognize private protocols.
    /// * `len` - Number of struct fields supplied by Serde.
    ///
    /// # Returns
    ///
    /// A private-protocol compound for recognized protocol names, or a map
    /// compound with bounded initial field capacity.
    fn serialize_struct(self, name: &'static str, len: usize) -> Result<Self::SerializeStruct, Self::Error> {
        if let Some(kind) = JsonPrivateProtocolKind::from_struct_name(name) {
            let state = JsonPrivateProtocolState::new(kind, len).map_err(|reason| {
                JsonSerializationError::new(JsonSerializationErrorKind::InvalidSerializerState { reason })
            })?;
            Ok(JsonValueCompound::private(kind, state))
        } else {
            Ok(JsonValueCompound::map(preallocated_capacity(Some(len))))
        }
    }

    /// Creates a struct-variant serializer with bounded initial capacity.
    ///
    /// # Parameters
    ///
    /// * `_name` - Serde enum type name, unused by JSON value encoding.
    /// * `_variant_index` - Serde variant index, unused by JSON value encoding.
    /// * `variant` - Variant name used as the outer object key.
    /// * `len` - Number of struct fields supplied by Serde.
    ///
    /// # Returns
    ///
    /// A struct-variant compound with bounded initial field capacity.
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
    ///
    /// # Errors
    ///
    /// Returns a display formatting error if formatting `value` fails.
    ///
    /// # Type Parameters
    ///
    /// * `T` - Displayable value type, which may be unsized.
    ///
    /// # Parameters
    ///
    /// * `value` - Value whose display representation becomes the JSON string.
    ///
    /// # Returns
    ///
    /// A JSON string containing the formatted value.
    fn collect_str<T>(self, value: &T) -> Result<Value, Self::Error>
    where
        T: Display + ?Sized,
    {
        let mut text = String::new();
        write!(&mut text, "{value}")
            .map_err(|_| JsonSerializationError::new(JsonSerializationErrorKind::DisplayFormattingFailed))?;
        Ok(Value::String(text))
    }
}
