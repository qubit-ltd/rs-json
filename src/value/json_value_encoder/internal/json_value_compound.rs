// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Compound Serde serializer states for strict JSON values.

use serde::Serialize;
use serde::Serializer;
use serde::ser::Impossible;
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
use super::json_value_serializer::decode_raw_value;
use crate::encode::JsonSerializationError;
use crate::encode::JsonSerializationErrorKind;
use crate::encode::JsonSerializerStateError;
use crate::internal::JsonPrivateProtocolKind;
use crate::internal::JsonPrivateProtocolState;

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
        /// Validated private protocol lifecycle.
        state: JsonPrivateProtocolState,
    },
    /// Text carried by serde_json's private Number protocol.
    Number {
        /// Number lexeme emitted by the single protocol field.
        text: Option<String>,
        /// Validated private protocol lifecycle.
        state: JsonPrivateProtocolState,
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
    pub(in crate::value::json_value_encoder) fn tuple_variant(
        variant: &str,
        capacity: usize,
    ) -> Self {
        Self::TupleVariant {
            variant: variant.to_owned(),
            values: Vec::with_capacity(capacity),
        }
    }

    /// Creates a struct variant with reserved field storage.
    #[inline]
    #[must_use]
    pub(in crate::value::json_value_encoder) fn struct_variant(
        variant: &str,
        capacity: usize,
    ) -> Self {
        Self::StructVariant {
            variant: variant.to_owned(),
            values: Map::with_capacity(capacity),
        }
    }

    /// Creates an empty RawValue protocol state.
    #[inline(always)]
    #[must_use]
    pub(in crate::value::json_value_encoder) const fn private(
        kind: JsonPrivateProtocolKind,
        state: JsonPrivateProtocolState,
    ) -> Self {
        match kind {
            JsonPrivateProtocolKind::RawValue => Self::RawValue { text: None, state },
            JsonPrivateProtocolKind::Number => Self::Number { text: None, state },
        }
    }

    /// Validates and stores one private protocol text field.
    fn set_private_payload<T>(
        text: &mut Option<String>,
        state: &mut JsonPrivateProtocolState,
        kind: JsonPrivateProtocolKind,
        key: &'static str,
        value: &T,
    ) -> Result<(), JsonSerializationError>
    where
        T: Serialize + ?Sized,
    {
        if state.begin(key).is_err() {
            return Err(private_invalid_state(kind));
        }
        let value = match value.serialize(PrivateTextSerializer { kind }) {
            Ok(Value::String(value)) => value,
            Ok(_) => {
                state.fail();
                return Err(private_invalid_state(kind));
            }
            Err(error) => {
                state.fail();
                return Err(error);
            }
        };
        *text = Some(value);
        state.complete().map_err(|_| private_invalid_state(kind))
    }

    /// Inserts one unique object entry into the supplied map.
    fn insert(
        values: &mut Map<String, Value>,
        key: String,
        value: Value,
    ) -> Result<(), JsonSerializationError> {
        if values.contains_key(&key) {
            return Err(JsonSerializationError::new(
                JsonSerializationErrorKind::DuplicateObjectKey,
            ));
        }
        values.insert(key, value);
        Ok(())
    }
}

impl SerializeSeq for JsonValueCompound {
    type Ok = Value;
    type Error = JsonSerializationError;

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
    type Error = JsonSerializationError;

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
    type Error = JsonSerializationError;

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
    type Error = JsonSerializationError;

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
    type Error = JsonSerializationError;

    /// Serializes and retains the next map key.
    fn serialize_key<T>(&mut self, key: &T) -> Result<(), Self::Error>
    where
        T: Serialize + ?Sized,
    {
        let Self::Map { next_key, .. } = self else {
            return Err(invalid_state(JsonSerializerStateError::UnexpectedCompound));
        };
        if next_key.is_some() {
            return Err(invalid_state(
                JsonSerializerStateError::MapKeyAlreadyPending,
            ));
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
            return Err(invalid_state(
                JsonSerializerStateError::MapEndedWithPendingKey,
            ));
        }
        Ok(Value::Object(values))
    }
}

impl SerializeStruct for JsonValueCompound {
    type Ok = Value;
    type Error = JsonSerializationError;

    /// Rejects skipped fields in either serde_json private protocol.
    fn skip_field(&mut self, _key: &'static str) -> Result<(), Self::Error> {
        match self {
            Self::RawValue { state, .. } => {
                state.fail();
                Err(private_invalid_state(JsonPrivateProtocolKind::RawValue))
            }
            Self::Number { state, .. } => {
                state.fail();
                Err(private_invalid_state(JsonPrivateProtocolKind::Number))
            }
            _ => Ok(()),
        }
    }

    /// Serializes one unique named struct field.
    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<(), Self::Error>
    where
        T: Serialize + ?Sized,
    {
        match self {
            Self::Map { values, next_key } => {
                if next_key.is_some() {
                    return Err(invalid_state(
                        JsonSerializerStateError::MapKeyAlreadyPending,
                    ));
                }
                Self::insert(
                    values,
                    key.to_owned(),
                    value.serialize(JsonValueSerializer)?,
                )
            }
            Self::RawValue { text, state } => Self::set_private_payload(
                text,
                state,
                JsonPrivateProtocolKind::RawValue,
                key,
                value,
            ),
            Self::Number { text, state } => {
                Self::set_private_payload(text, state, JsonPrivateProtocolKind::Number, key, value)
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
                    return Err(invalid_state(
                        JsonSerializerStateError::MapEndedWithPendingKey,
                    ));
                }
                Ok(Value::Object(values))
            }
            Self::RawValue {
                text: Some(text),
                state,
            } => {
                state
                    .finish()
                    .map_err(|_| private_invalid_state(JsonPrivateProtocolKind::RawValue))?;
                decode_raw_value(&text)
            }
            Self::RawValue { state, .. } => Err(state.finish().err().map_or_else(
                || private_invalid_state(JsonPrivateProtocolKind::RawValue),
                |_| private_invalid_state(JsonPrivateProtocolKind::RawValue),
            )),
            Self::Number {
                text: Some(text),
                state,
            } => {
                state
                    .finish()
                    .map_err(|_| private_invalid_state(JsonPrivateProtocolKind::Number))?;
                crate::internal::StrictJsonNumber::parse(&text).map(Value::Number)
            }
            Self::Number { state, .. } => Err(state.finish().err().map_or_else(
                || private_invalid_state(JsonPrivateProtocolKind::Number),
                |_| private_invalid_state(JsonPrivateProtocolKind::Number),
            )),
            _ => Err(invalid_state(JsonSerializerStateError::UnexpectedCompound)),
        }
    }
}

impl SerializeStructVariant for JsonValueCompound {
    type Ok = Value;
    type Error = JsonSerializationError;

    /// Serializes one unique named struct-variant field.
    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<(), Self::Error>
    where
        T: Serialize + ?Sized,
    {
        let Self::StructVariant { values, .. } = self else {
            return Err(invalid_state(JsonSerializerStateError::UnexpectedCompound));
        };
        Self::insert(
            values,
            key.to_owned(),
            value.serialize(JsonValueSerializer)?,
        )
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
fn invalid_state(reason: JsonSerializerStateError) -> JsonSerializationError {
    JsonSerializationError::new(JsonSerializationErrorKind::InvalidSerializerState { reason })
}

/// Creates the stable error for one private serde_json protocol.
fn private_invalid_state(kind: JsonPrivateProtocolKind) -> JsonSerializationError {
    invalid_state(kind.invalid_state())
}

/// Serializer that accepts only string and Display based private payloads.
struct PrivateTextSerializer {
    kind: JsonPrivateProtocolKind,
}

impl Serializer for PrivateTextSerializer {
    type Ok = Value;
    type Error = JsonSerializationError;
    type SerializeSeq = Impossible<Value, JsonSerializationError>;
    type SerializeTuple = Impossible<Value, JsonSerializationError>;
    type SerializeTupleStruct = Impossible<Value, JsonSerializationError>;
    type SerializeTupleVariant = Impossible<Value, JsonSerializationError>;
    type SerializeMap = Impossible<Value, JsonSerializationError>;
    type SerializeStruct = Impossible<Value, JsonSerializationError>;
    type SerializeStructVariant = Impossible<Value, JsonSerializationError>;

    fn serialize_bool(self, _: bool) -> Result<Value, Self::Error> {
        Err(private_invalid_state(self.kind))
    }
    fn serialize_i8(self, _: i8) -> Result<Value, Self::Error> {
        Err(private_invalid_state(self.kind))
    }
    fn serialize_i16(self, _: i16) -> Result<Value, Self::Error> {
        Err(private_invalid_state(self.kind))
    }
    fn serialize_i32(self, _: i32) -> Result<Value, Self::Error> {
        Err(private_invalid_state(self.kind))
    }
    fn serialize_i64(self, _: i64) -> Result<Value, Self::Error> {
        Err(private_invalid_state(self.kind))
    }
    fn serialize_i128(self, _: i128) -> Result<Value, Self::Error> {
        Err(private_invalid_state(self.kind))
    }
    fn serialize_u8(self, _: u8) -> Result<Value, Self::Error> {
        Err(private_invalid_state(self.kind))
    }
    fn serialize_u16(self, _: u16) -> Result<Value, Self::Error> {
        Err(private_invalid_state(self.kind))
    }
    fn serialize_u32(self, _: u32) -> Result<Value, Self::Error> {
        Err(private_invalid_state(self.kind))
    }
    fn serialize_u64(self, _: u64) -> Result<Value, Self::Error> {
        Err(private_invalid_state(self.kind))
    }
    fn serialize_u128(self, _: u128) -> Result<Value, Self::Error> {
        Err(private_invalid_state(self.kind))
    }
    fn serialize_f32(self, _: f32) -> Result<Value, Self::Error> {
        Err(private_invalid_state(self.kind))
    }
    fn serialize_f64(self, _: f64) -> Result<Value, Self::Error> {
        Err(private_invalid_state(self.kind))
    }
    fn serialize_char(self, _: char) -> Result<Value, Self::Error> {
        Err(private_invalid_state(self.kind))
    }

    fn serialize_str(self, value: &str) -> Result<Value, Self::Error> {
        Ok(Value::String(value.to_owned()))
    }

    fn serialize_bytes(self, _: &[u8]) -> Result<Value, Self::Error> {
        Err(private_invalid_state(self.kind))
    }
    fn serialize_none(self) -> Result<Value, Self::Error> {
        Err(private_invalid_state(self.kind))
    }
    fn serialize_some<T: Serialize + ?Sized>(self, _: &T) -> Result<Value, Self::Error> {
        Err(private_invalid_state(self.kind))
    }
    fn serialize_unit(self) -> Result<Value, Self::Error> {
        Err(private_invalid_state(self.kind))
    }
    fn serialize_unit_struct(self, _: &'static str) -> Result<Value, Self::Error> {
        Err(private_invalid_state(self.kind))
    }
    fn serialize_unit_variant(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
    ) -> Result<Value, Self::Error> {
        Err(private_invalid_state(self.kind))
    }
    fn serialize_newtype_struct<T: Serialize + ?Sized>(
        self,
        _: &'static str,
        _: &T,
    ) -> Result<Value, Self::Error> {
        Err(private_invalid_state(self.kind))
    }
    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: &T,
    ) -> Result<Value, Self::Error> {
        Err(private_invalid_state(self.kind))
    }
    fn serialize_seq(self, _: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        Err(private_invalid_state(self.kind))
    }
    fn serialize_tuple(self, _: usize) -> Result<Self::SerializeTuple, Self::Error> {
        Err(private_invalid_state(self.kind))
    }
    fn serialize_tuple_struct(
        self,
        _: &'static str,
        _: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        Err(private_invalid_state(self.kind))
    }
    fn serialize_tuple_variant(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        Err(private_invalid_state(self.kind))
    }
    fn serialize_map(self, _: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        Err(private_invalid_state(self.kind))
    }
    fn serialize_struct(
        self,
        _: &'static str,
        _: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        Err(private_invalid_state(self.kind))
    }
    fn serialize_struct_variant(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        Err(private_invalid_state(self.kind))
    }

    fn collect_str<T: std::fmt::Display + ?Sized>(self, value: &T) -> Result<Value, Self::Error> {
        let mut text = String::new();
        std::fmt::write(&mut text, format_args!("{value}")).map_err(|_| {
            JsonSerializationError::new(JsonSerializationErrorKind::DisplayFormattingFailed)
        })?;
        Ok(Value::String(text))
    }
}
