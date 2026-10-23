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

use serde::Serialize;
use serde::Serializer;
use serde::ser::Impossible;
use serde_json::Number;
use serde_json::to_string;

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
            #[doc = ""]
            #[doc = "# Parameters"]
            #[doc = ""]
            #[doc = "* `self` — the serializer instance."]
            #[doc = "* `value` — the integer key to serialize."]
            #[doc = ""]
            #[doc = "# Returns"]
            #[doc = ""]
            #[doc = "The key's canonical decimal representation."]
            #[doc = ""]
            #[doc = "# Errors"]
            #[doc = ""]
            #[doc = "This conversion is infallible."]
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
    ///
    /// # Parameters
    ///
    /// * `self` - Serializer instance consumed for this key.
    /// * `value` - Boolean key to encode.
    ///
    /// # Returns
    ///
    /// The key's canonical `true` or `false` text.
    ///
    /// # Errors
    ///
    /// This conversion is infallible.
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
    ///
    /// # Parameters
    ///
    /// * `self` - Serializer instance consumed for this key.
    /// * `value` - Signed integer key.
    ///
    /// # Returns
    ///
    /// The signed integer in canonical decimal text.
    ///
    /// # Errors
    ///
    /// This conversion is infallible.
    fn serialize_i128(self, value: i128) -> Result<String, Self::Error> {
        Ok(JsonMapKey::signed_wide(value))
    }

    /// Serializes a full-range unsigned integer key as canonical decimal text.
    ///
    /// # Parameters
    ///
    /// * `self` - Serializer instance consumed for this key.
    /// * `value` - Unsigned integer key.
    ///
    /// # Returns
    ///
    /// The unsigned integer in canonical decimal text.
    ///
    /// # Errors
    ///
    /// This conversion is infallible.
    fn serialize_u128(self, value: u128) -> Result<String, Self::Error> {
        Ok(JsonMapKey::unsigned_wide(value))
    }

    /// Serializes a finite 32-bit floating-point key.
    ///
    /// # Parameters
    ///
    /// * `self` - Serializer instance consumed for this key.
    /// * `value` - 32-bit floating-point key.
    ///
    /// # Returns
    ///
    /// The finite value in serde_json's f32 JSON representation.
    ///
    /// # Errors
    ///
    /// Returns `NonFiniteFloat` for a non-finite input or
    /// `InvalidNumberRepresentation` when serde_json cannot format the value.
    fn serialize_f32(self, value: f32) -> Result<String, Self::Error> {
        if !value.is_finite() {
            return Err(JsonSerializationError::new(JsonSerializationErrorKind::NonFiniteFloat));
        }
        to_string(&value)
            .map_err(|_| JsonSerializationError::new(JsonSerializationErrorKind::InvalidNumberRepresentation))
    }

    /// Serializes a finite 64-bit floating-point key.
    ///
    /// # Parameters
    ///
    /// * `self` - Serializer instance consumed for this key.
    /// * `value` - 64-bit floating-point key.
    ///
    /// # Returns
    ///
    /// The finite value in canonical decimal text.
    ///
    /// # Errors
    ///
    /// Returns `NonFiniteFloat` when the input is non-finite.
    fn serialize_f64(self, value: f64) -> Result<String, Self::Error> {
        Number::from_f64(value)
            .map(|number| number.to_string())
            .ok_or_else(|| JsonSerializationError::new(JsonSerializationErrorKind::NonFiniteFloat))
    }

    /// Serializes a character key.
    ///
    /// # Parameters
    ///
    /// * `self` - Serializer instance consumed for this key.
    /// * `value` - Character key.
    ///
    /// # Returns
    ///
    /// The character encoded as a one-character string.
    ///
    /// # Errors
    ///
    /// This conversion is infallible.
    #[inline]
    fn serialize_char(self, value: char) -> Result<String, Self::Error> {
        Ok(value.to_string())
    }

    /// Copies a string key.
    ///
    /// # Parameters
    ///
    /// * `self` - Serializer instance consumed for this key.
    /// * `value` - String slice copied into the owned key.
    ///
    /// # Returns
    ///
    /// An owned copy of the input string.
    ///
    /// # Errors
    ///
    /// This conversion is infallible.
    #[inline]
    fn serialize_str(self, value: &str) -> Result<String, Self::Error> {
        Ok(value.to_owned())
    }

    /// Rejects byte sequences as object keys.
    ///
    /// # Parameters
    ///
    /// * `self` - Serializer instance consumed for this key.
    /// * `_value` - Byte sequence rejected as a key.
    ///
    /// # Returns
    ///
    /// No value; byte keys are always rejected.
    ///
    /// # Errors
    ///
    /// Returns `UnsupportedMapKey { kind: Bytes }`.
    fn serialize_bytes(self, _value: &[u8]) -> Result<String, Self::Error> {
        Err(unsupported_key(JsonMapKeyKind::Bytes))
    }

    /// Rejects absent optional keys.
    ///
    /// # Parameters
    ///
    /// * `self` - Serializer instance consumed for this key.
    ///
    /// # Returns
    ///
    /// No value; absent optional keys are always rejected.
    ///
    /// # Errors
    ///
    /// Returns `UnsupportedMapKey { kind: None }`.
    fn serialize_none(self) -> Result<String, Self::Error> {
        Err(unsupported_key(JsonMapKeyKind::None))
    }

    /// Delegates a present optional key to its wrapped value.
    ///
    /// # Type Parameters
    ///
    /// * `T` - Wrapped value type.
    ///
    /// # Parameters
    ///
    /// * `self` - Serializer instance consumed for this key.
    /// * `value` - Wrapped value serialized as the key.
    ///
    /// # Returns
    ///
    /// The result of serializing the wrapped key value.
    ///
    /// # Errors
    ///
    /// Returns the wrapped serializer's error when that key shape is
    /// unsupported or invalid.
    fn serialize_some<T>(self, value: &T) -> Result<String, Self::Error>
    where
        T: Serialize + ?Sized,
    {
        value.serialize(self)
    }

    /// Rejects unit keys.
    ///
    /// # Parameters
    ///
    /// * `self` - Serializer instance consumed for this key.
    ///
    /// # Returns
    ///
    /// No value; unit keys are always rejected.
    ///
    /// # Errors
    ///
    /// Returns `UnsupportedMapKey { kind: Unit }`.
    fn serialize_unit(self) -> Result<String, Self::Error> {
        Err(unsupported_key(JsonMapKeyKind::Unit))
    }

    /// Rejects unit-struct keys.
    ///
    /// # Parameters
    ///
    /// * `self` - Serializer instance consumed for this key.
    /// * `_name` - Unit-struct name; the key shape is rejected regardless of
    ///   it.
    ///
    /// # Returns
    ///
    /// No value; unit-struct keys are always rejected.
    ///
    /// # Errors
    ///
    /// Returns `UnsupportedMapKey { kind: UnitStruct }`.
    fn serialize_unit_struct(self, _name: &'static str) -> Result<String, Self::Error> {
        Err(unsupported_key(JsonMapKeyKind::UnitStruct))
    }

    /// Serializes a unit variant through its variant name.
    ///
    /// # Parameters
    ///
    /// * `self` - Serializer instance consumed for this key.
    /// * `_name` - Enum name, unused because the key uses only the variant
    ///   name.
    /// * `_variant_index` - Variant index, unused because the key uses only the
    ///   variant name.
    /// * `variant` - Variant name copied into the key.
    ///
    /// # Returns
    ///
    /// An owned copy of the enum variant name.
    ///
    /// # Errors
    ///
    /// This conversion is infallible.
    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<String, Self::Error> {
        Ok(variant.to_owned())
    }

    /// Delegates a newtype-struct key to its wrapped value.
    ///
    /// # Type Parameters
    ///
    /// * `T` - Wrapped value type.
    ///
    /// # Parameters
    ///
    /// * `self` - Serializer instance consumed for this key.
    /// * `_name` - Newtype-struct name, not part of the serialized key.
    /// * `value` - Wrapped value serialized as the key.
    ///
    /// # Returns
    ///
    /// The result of serializing the wrapped key value.
    ///
    /// # Errors
    ///
    /// Returns the wrapped serializer's error when that key shape is
    /// unsupported or invalid.
    fn serialize_newtype_struct<T>(self, _name: &'static str, value: &T) -> Result<String, Self::Error>
    where
        T: Serialize + ?Sized,
    {
        value.serialize(self)
    }

    /// Rejects newtype-variant keys.
    ///
    /// # Type Parameters
    ///
    /// * `T` - Wrapped value type.
    ///
    /// # Parameters
    ///
    /// * `self` - Serializer instance consumed for this key.
    /// * `_name` - Enum name for the rejected shape.
    /// * `_variant_index` - Variant index for the rejected shape.
    /// * `_variant` - Variant name for the rejected shape.
    /// * `_value` - Wrapped value for the rejected shape.
    ///
    /// # Returns
    ///
    /// No value; newtype-variant keys are always rejected.
    ///
    /// # Errors
    ///
    /// Returns `UnsupportedMapKey { kind: NewtypeVariant }`.
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
    ///
    /// # Parameters
    ///
    /// * `self` - Serializer instance consumed for this key.
    /// * `_len` - Optional sequence length; sequence keys are rejected.
    ///
    /// # Returns
    ///
    /// No value; the `Impossible` associated serializer has no constructible
    /// success result.
    ///
    /// # Errors
    ///
    /// Returns `UnsupportedMapKey { kind: Sequence }`.
    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        Err(unsupported_key(JsonMapKeyKind::Sequence))
    }

    /// Rejects tuple keys.
    ///
    /// # Parameters
    ///
    /// * `self` - Serializer instance consumed for this key.
    /// * `_len` - Tuple length; tuple keys are rejected.
    ///
    /// # Returns
    ///
    /// No value; the `Impossible` associated serializer has no constructible
    /// success result.
    ///
    /// # Errors
    ///
    /// Returns `UnsupportedMapKey { kind: Tuple }`.
    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        Err(unsupported_key(JsonMapKeyKind::Tuple))
    }

    /// Rejects tuple-struct keys.
    ///
    /// # Parameters
    ///
    /// * `self` - Serializer instance consumed for this key.
    /// * `_name` - Tuple-struct name for the rejected shape.
    /// * `_len` - Tuple-struct field count for the rejected shape.
    ///
    /// # Returns
    ///
    /// No value; the `Impossible` associated serializer has no constructible
    /// success result.
    ///
    /// # Errors
    ///
    /// Returns `UnsupportedMapKey { kind: TupleStruct }`.
    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        Err(unsupported_key(JsonMapKeyKind::TupleStruct))
    }

    /// Rejects tuple-variant keys.
    ///
    /// # Parameters
    ///
    /// * `self` - Serializer instance consumed for this key.
    /// * `_name` - Enum name for the rejected shape.
    /// * `_variant_index` - Variant index for the rejected shape.
    /// * `_variant` - Variant name for the rejected shape.
    /// * `_len` - Variant field count for the rejected shape.
    ///
    /// # Returns
    ///
    /// No value; the `Impossible` associated serializer has no constructible
    /// success result.
    ///
    /// # Errors
    ///
    /// Returns `UnsupportedMapKey { kind: TupleVariant }`.
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
    ///
    /// # Parameters
    ///
    /// * `self` - Serializer instance consumed for this key.
    /// * `_len` - Optional map length; map keys are rejected.
    ///
    /// # Returns
    ///
    /// No value; the `Impossible` associated serializer has no constructible
    /// success result.
    ///
    /// # Errors
    ///
    /// Returns `UnsupportedMapKey { kind: Map }`.
    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        Err(unsupported_key(JsonMapKeyKind::Map))
    }

    /// Rejects struct keys.
    ///
    /// # Parameters
    ///
    /// * `self` - Serializer instance consumed for this key.
    /// * `_name` - Struct name for the rejected shape.
    /// * `_len` - Struct field count for the rejected shape.
    ///
    /// # Returns
    ///
    /// No value; the `Impossible` associated serializer has no constructible
    /// success result.
    ///
    /// # Errors
    ///
    /// Returns `UnsupportedMapKey { kind: Struct }`.
    fn serialize_struct(self, _name: &'static str, _len: usize) -> Result<Self::SerializeStruct, Self::Error> {
        Err(unsupported_key(JsonMapKeyKind::Struct))
    }

    /// Rejects struct-variant keys.
    ///
    /// # Parameters
    ///
    /// * `self` - Serializer instance consumed for this key.
    /// * `_name` - Enum name for the rejected shape.
    /// * `_variant_index` - Variant index for the rejected shape.
    /// * `_variant` - Variant name for the rejected shape.
    /// * `_len` - Variant field count for the rejected shape.
    ///
    /// # Returns
    ///
    /// No value; the `Impossible` associated serializer has no constructible
    /// success result.
    ///
    /// # Errors
    ///
    /// Returns `UnsupportedMapKey { kind: StructVariant }`.
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
    ///
    /// # Type Parameters
    ///
    /// * `T` - Displayable key type.
    ///
    /// # Parameters
    ///
    /// * `self` - Serializer instance consumed for this key.
    /// * `value` - Displayable value formatted into the key text.
    ///
    /// # Returns
    ///
    /// The text produced by formatting the value with `Display`.
    ///
    /// # Errors
    ///
    /// Returns `DisplayFormattingFailed` when the formatter rejects a write.
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

/// Builds a typed rejection error from the key shape without retaining the key
/// value.
///
/// # Parameters
///
/// * `kind` - Unsupported scalar or compound key shape to record in the error.
///
/// # Returns
///
/// A stable `UnsupportedMapKey` serialization error containing only `kind`.
fn unsupported_key(kind: JsonMapKeyKind) -> JsonSerializationError {
    JsonSerializationError::new(JsonSerializationErrorKind::UnsupportedMapKey { kind })
}
