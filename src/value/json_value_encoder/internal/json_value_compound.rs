// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Compound Serde serializer states for strict JSON values.

use serde::Serialize;
use serde::ser::SerializeMap;
use serde::ser::SerializeSeq;
use serde::ser::SerializeStruct;
use serde::ser::SerializeStructVariant;
use serde::ser::SerializeTuple;
use serde::ser::SerializeTupleStruct;
use serde::ser::SerializeTupleVariant;
use serde_json::Map;
use serde_json::Value;

use super::JsonValueMapKeySerializer;
use super::JsonValueSerializer;
use super::json_value_serializer::RAW_VALUE_TOKEN;
use super::json_value_serializer::decode_raw_value;
use crate::value::JsonSerializerStateError;
use crate::value::JsonValueEncodeError;
use crate::value::JsonValueEncodeErrorKind;

/// Accumulates one Serde compound value in its JSON representation state.
pub(in crate::value::json_value_encoder) enum JsonValueCompound {
    /// Array-like sequence or tuple values.
    Sequence(
        /// Values accumulated in encounter order.
        Vec<Value>,
    ),
    /// Object entries and a key awaiting its value.
    Map {
        /// Completed unique object entries.
        values: Map<String, Value>,
        /// Serialized key supplied through split map calls.
        next_key: Option<String>,
    },
    /// Tuple variant fields nested below one variant name.
    TupleVariant {
        /// Outer JSON object key.
        variant: String,
        /// Ordered tuple fields.
        values: Vec<Value>,
    },
    /// Struct variant fields nested below one variant name.
    StructVariant {
        /// Outer JSON object key.
        variant: String,
        /// Unique named struct fields.
        values: Map<String, Value>,
    },
    /// Text carried by serde_json's RawValue struct protocol.
    RawValue {
        /// Raw JSON text emitted by the single protocol field.
        text: Option<String>,
    },
}

impl JsonValueCompound {
    /// Creates an array-like compound with reserved storage.
    #[inline]
    #[must_use]
    pub(in crate::value::json_value_encoder) fn sequence(capacity: usize) -> Self {
        Self::Sequence(Vec::with_capacity(capacity))
    }

    /// Creates an object-like compound with reserved storage.
    #[inline]
    #[must_use]
    pub(in crate::value::json_value_encoder) fn map(capacity: usize) -> Self {
        Self::Map {
            values: Map::with_capacity(capacity),
            next_key: None,
        }
    }

    /// Creates a tuple variant with reserved field storage.
    #[inline]
    #[must_use]
    pub(in crate::value::json_value_encoder) fn tuple_variant(variant: &str, capacity: usize) -> Self {
        Self::TupleVariant {
            variant: variant.to_owned(),
            values: Vec::with_capacity(capacity),
        }
    }

    /// Creates a struct variant with reserved field storage.
    #[inline]
    #[must_use]
    pub(in crate::value::json_value_encoder) fn struct_variant(variant: &str, capacity: usize) -> Self {
        Self::StructVariant {
            variant: variant.to_owned(),
            values: Map::with_capacity(capacity),
        }
    }

    /// Creates an empty RawValue protocol state.
    #[inline(always)]
    #[must_use]
    pub(in crate::value::json_value_encoder) const fn raw_value() -> Self {
        Self::RawValue { text: None }
    }

    /// Inserts one unique object entry into the supplied map.
    fn insert(values: &mut Map<String, Value>, key: String, value: Value) -> Result<(), JsonValueEncodeError> {
        if values.contains_key(&key) {
            return Err(JsonValueEncodeError::new(JsonValueEncodeErrorKind::DuplicateObjectKey));
        }
        values.insert(key, value);
        Ok(())
    }
}

impl SerializeSeq for JsonValueCompound {
    type Ok = Value;
    type Error = JsonValueEncodeError;

    /// Serializes and appends one sequence element.
    fn serialize_element<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: Serialize + ?Sized,
    {
        let Self::Sequence(values) = self else {
            return Err(invalid_state(JsonSerializerStateError::UnexpectedCompound));
        };
        values.push(value.serialize(JsonValueSerializer)?);
        Ok(())
    }

    /// Returns the accumulated JSON array.
    fn end(self) -> Result<Value, Self::Error> {
        let Self::Sequence(values) = self else {
            return Err(invalid_state(JsonSerializerStateError::UnexpectedCompound));
        };
        Ok(Value::Array(values))
    }
}

impl SerializeTuple for JsonValueCompound {
    type Ok = Value;
    type Error = JsonValueEncodeError;

    /// Serializes and appends one tuple element.
    #[inline(always)]
    fn serialize_element<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: Serialize + ?Sized,
    {
        SerializeSeq::serialize_element(self, value)
    }

    /// Returns the accumulated tuple as a JSON array.
    #[inline(always)]
    fn end(self) -> Result<Value, Self::Error> {
        SerializeSeq::end(self)
    }
}

impl SerializeTupleStruct for JsonValueCompound {
    type Ok = Value;
    type Error = JsonValueEncodeError;

    /// Serializes and appends one tuple-struct field.
    #[inline(always)]
    fn serialize_field<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: Serialize + ?Sized,
    {
        SerializeSeq::serialize_element(self, value)
    }

    /// Returns the accumulated tuple struct as a JSON array.
    #[inline(always)]
    fn end(self) -> Result<Value, Self::Error> {
        SerializeSeq::end(self)
    }
}

impl SerializeTupleVariant for JsonValueCompound {
    type Ok = Value;
    type Error = JsonValueEncodeError;

    /// Serializes and appends one tuple variant field.
    fn serialize_field<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: Serialize + ?Sized,
    {
        let Self::TupleVariant { values, .. } = self else {
            return Err(invalid_state(JsonSerializerStateError::UnexpectedCompound));
        };
        values.push(value.serialize(JsonValueSerializer)?);
        Ok(())
    }

    /// Returns the tuple variant as a single-key object.
    fn end(self) -> Result<Value, Self::Error> {
        let Self::TupleVariant { variant, values } = self else {
            return Err(invalid_state(JsonSerializerStateError::UnexpectedCompound));
        };
        let mut object = Map::new();
        object.insert(variant, Value::Array(values));
        Ok(Value::Object(object))
    }
}

impl SerializeMap for JsonValueCompound {
    type Ok = Value;
    type Error = JsonValueEncodeError;

    /// Serializes and retains the next map key.
    fn serialize_key<T>(&mut self, key: &T) -> Result<(), Self::Error>
    where
        T: Serialize + ?Sized,
    {
        let Self::Map { next_key, .. } = self else {
            return Err(invalid_state(JsonSerializerStateError::UnexpectedCompound));
        };
        if next_key.is_some() {
            return Err(invalid_state(JsonSerializerStateError::MapKeyAlreadyPending));
        }
        *next_key = Some(key.serialize(JsonValueMapKeySerializer)?);
        Ok(())
    }

    /// Serializes the value paired with the retained map key.
    fn serialize_value<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: Serialize + ?Sized,
    {
        let Self::Map { values, next_key } = self else {
            return Err(invalid_state(JsonSerializerStateError::UnexpectedCompound));
        };
        let key = next_key
            .take()
            .ok_or_else(|| invalid_state(JsonSerializerStateError::MapValueWithoutKey))?;
        Self::insert(values, key, value.serialize(JsonValueSerializer)?)
    }

    /// Returns the completed object after checking split map state.
    fn end(self) -> Result<Value, Self::Error> {
        let Self::Map { values, next_key } = self else {
            return Err(invalid_state(JsonSerializerStateError::UnexpectedCompound));
        };
        if next_key.is_some() {
            return Err(invalid_state(JsonSerializerStateError::MapEndedWithPendingKey));
        }
        Ok(Value::Object(values))
    }
}

impl SerializeStruct for JsonValueCompound {
    type Ok = Value;
    type Error = JsonValueEncodeError;

    /// Serializes one unique named struct field.
    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<(), Self::Error>
    where
        T: Serialize + ?Sized,
    {
        match self {
            Self::Map { values, next_key } => {
                if next_key.is_some() {
                    return Err(invalid_state(JsonSerializerStateError::MapKeyAlreadyPending));
                }
                Self::insert(values, key.to_owned(), value.serialize(JsonValueSerializer)?)
            }
            Self::RawValue { text } => {
                if key != RAW_VALUE_TOKEN || text.is_some() {
                    return Err(invalid_state(JsonSerializerStateError::InvalidRawValueProtocol));
                }
                let Value::String(value) = value.serialize(JsonValueSerializer)? else {
                    return Err(invalid_state(JsonSerializerStateError::InvalidRawValueProtocol));
                };
                *text = Some(value);
                Ok(())
            }
            _ => Err(invalid_state(JsonSerializerStateError::UnexpectedCompound)),
        }
    }

    /// Returns the completed struct as a JSON object.
    #[inline(always)]
    fn end(self) -> Result<Value, Self::Error> {
        match self {
            Self::Map { values, next_key } => {
                if next_key.is_some() {
                    return Err(invalid_state(JsonSerializerStateError::MapEndedWithPendingKey));
                }
                Ok(Value::Object(values))
            }
            Self::RawValue { text: Some(text) } => decode_raw_value(&text),
            Self::RawValue { .. } => Err(invalid_state(JsonSerializerStateError::InvalidRawValueProtocol)),
            _ => Err(invalid_state(JsonSerializerStateError::UnexpectedCompound)),
        }
    }
}

impl SerializeStructVariant for JsonValueCompound {
    type Ok = Value;
    type Error = JsonValueEncodeError;

    /// Serializes one unique named struct-variant field.
    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<(), Self::Error>
    where
        T: Serialize + ?Sized,
    {
        let Self::StructVariant { values, .. } = self else {
            return Err(invalid_state(JsonSerializerStateError::UnexpectedCompound));
        };
        Self::insert(values, key.to_owned(), value.serialize(JsonValueSerializer)?)
    }

    /// Returns the struct variant as a nested single-key object.
    fn end(self) -> Result<Value, Self::Error> {
        let Self::StructVariant { variant, values } = self else {
            return Err(invalid_state(JsonSerializerStateError::UnexpectedCompound));
        };
        let mut object = Map::new();
        object.insert(variant, Value::Object(values));
        Ok(Value::Object(object))
    }
}

/// Creates one privacy-safe compound-state failure.
#[inline(always)]
fn invalid_state(reason: JsonSerializerStateError) -> JsonValueEncodeError {
    JsonValueEncodeError::new(JsonValueEncodeErrorKind::InvalidSerializerState { reason })
}
