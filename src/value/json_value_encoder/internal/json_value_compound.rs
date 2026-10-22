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
use super::json_value_serializer::decode_raw_value;
use super::private_text_serializer::PrivateTextSerializer;
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
    ///
    /// # Parameters
    ///
    /// * `capacity` - Initial array element capacity.
    ///
    /// # Returns
    ///
    /// An empty sequence compound ready to collect values.
    #[inline]
    #[must_use]
    pub(in crate::value::json_value_encoder) fn sequence(capacity: usize) -> Self {
        Self::Sequence(Vec::with_capacity(capacity))
    }

    /// Creates an object-like compound with reserved storage.
    ///
    /// # Parameters
    ///
    /// * `capacity` - Initial object entry capacity.
    ///
    /// # Returns
    ///
    /// An empty map compound with no pending key.
    #[inline]
    #[must_use]
    pub(in crate::value::json_value_encoder) fn map(capacity: usize) -> Self {
        Self::Map {
            values: Map::with_capacity(capacity),
            next_key: None,
        }
    }

    /// Creates a tuple variant with reserved field storage.
    ///
    /// # Parameters
    ///
    /// * `variant` - Enum variant name used as the outer object key.
    /// * `capacity` - Initial tuple field capacity.
    ///
    /// # Returns
    ///
    /// An empty tuple-variant compound.
    #[inline]
    #[must_use]
    pub(in crate::value::json_value_encoder) fn tuple_variant(variant: &str, capacity: usize) -> Self {
        Self::TupleVariant {
            variant: variant.to_owned(),
            values: Vec::with_capacity(capacity),
        }
    }

    /// Creates a struct variant with reserved field storage.
    ///
    /// # Parameters
    ///
    /// * `variant` - Enum variant name used as the outer object key.
    /// * `capacity` - Initial named-field capacity.
    ///
    /// # Returns
    ///
    /// An empty struct-variant compound.
    #[inline]
    #[must_use]
    pub(in crate::value::json_value_encoder) fn struct_variant(variant: &str, capacity: usize) -> Self {
        Self::StructVariant {
            variant: variant.to_owned(),
            values: Map::with_capacity(capacity),
        }
    }

    /// Creates an empty RawValue protocol state.
    ///
    /// # Parameters
    ///
    /// * `kind` - Private serde_json protocol to track.
    /// * `state` - Initial validated protocol lifecycle state.
    ///
    /// # Returns
    ///
    /// An empty RawValue or Number compound matching `kind`.
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
    ///
    /// # Parameters
    ///
    /// * `text` - Slot that receives the serialized protocol payload.
    /// * `state` - Protocol lifecycle to advance after validation.
    /// * `kind` - Private serde_json protocol being decoded.
    /// * `key` - Struct field token supplied by the serializer.
    /// * `value` - Payload that must serialize as text.
    ///
    /// # Type Parameters
    ///
    /// * `T` - Payload type accepted by Serde.
    ///
    /// # Returns
    ///
    /// `Ok(())` after the payload has been serialized and the protocol state
    /// has advanced; otherwise an error is returned.
    ///
    /// # Errors
    ///
    /// Returns a serialization error if the lifecycle, field token, or payload
    /// does not match the selected private protocol.
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
    ///
    /// # Parameters
    ///
    /// * `values` - Object entries receiving the new pair.
    /// * `key` - Serialized object key to insert.
    /// * `value` - Materialized JSON value paired with the key.
    ///
    /// # Returns
    ///
    /// `Ok(())` after insertion succeeds.
    ///
    /// # Errors
    ///
    /// Returns `DuplicateObjectKey` when `key` is already present.
    fn insert(values: &mut Map<String, Value>, key: String, value: Value) -> Result<(), JsonSerializationError> {
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
    ///
    /// # Parameters
    ///
    /// * `value` - Element to serialize and append.
    ///
    /// # Type Parameters
    ///
    /// * `T` - Element type accepted by Serde.
    ///
    /// # Errors
    ///
    /// Returns an error if this is not a sequence compound or element
    /// serialization fails.
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
    ///
    /// # Returns
    ///
    /// The accumulated values as a JSON array.
    ///
    /// # Errors
    ///
    /// Returns an invalid-state error if this is not a sequence compound.
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
    ///
    /// # Parameters
    ///
    /// * `value` - Tuple element to serialize and append.
    ///
    /// # Type Parameters
    ///
    /// * `T` - Tuple element type accepted by Serde.
    ///
    /// # Errors
    ///
    /// Returns an error if element serialization or sequence-state validation
    /// fails.
    #[inline(always)]
    fn serialize_element<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: Serialize + ?Sized,
    {
        SerializeSeq::serialize_element(self, value)
    }

    /// Returns the accumulated tuple as a JSON array.
    ///
    /// # Returns
    ///
    /// The tuple fields as a JSON array.
    ///
    /// # Errors
    ///
    /// Returns an invalid-state error if this is not a sequence compound.
    #[inline(always)]
    fn end(self) -> Result<Value, Self::Error> {
        SerializeSeq::end(self)
    }
}

impl SerializeTupleStruct for JsonValueCompound {
    type Ok = Value;
    type Error = JsonSerializationError;

    /// Serializes and appends one tuple-struct field.
    ///
    /// # Parameters
    ///
    /// * `value` - Tuple-struct field to serialize and append.
    ///
    /// # Type Parameters
    ///
    /// * `T` - Field type accepted by Serde.
    ///
    /// # Errors
    ///
    /// Returns an error if field serialization or sequence-state validation
    /// fails.
    #[inline(always)]
    fn serialize_field<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: Serialize + ?Sized,
    {
        SerializeSeq::serialize_element(self, value)
    }

    /// Returns the accumulated tuple struct as a JSON array.
    ///
    /// # Returns
    ///
    /// The tuple-struct fields as a JSON array.
    ///
    /// # Errors
    ///
    /// Returns an invalid-state error if this is not a sequence compound.
    #[inline(always)]
    fn end(self) -> Result<Value, Self::Error> {
        SerializeSeq::end(self)
    }
}

impl SerializeTupleVariant for JsonValueCompound {
    type Ok = Value;
    type Error = JsonSerializationError;

    /// Serializes and appends one tuple variant field.
    ///
    /// # Parameters
    ///
    /// * `value` - Tuple variant field to serialize and append.
    ///
    /// # Type Parameters
    ///
    /// * `T` - Field type accepted by Serde.
    ///
    /// # Errors
    ///
    /// Returns an error if this is not a tuple variant or field serialization
    /// fails.
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
    ///
    /// # Returns
    ///
    /// An object mapping the variant name to its tuple array.
    ///
    /// # Errors
    ///
    /// Returns an invalid-state error if this is not a tuple variant.
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
    ///
    /// # Parameters
    ///
    /// * `key` - Map key to serialize using the JSON key contract.
    ///
    /// # Type Parameters
    ///
    /// * `T` - Key type accepted by Serde.
    ///
    /// # Errors
    ///
    /// Returns an error if this is not a map, another key is pending, or key
    /// serialization fails.
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
    ///
    /// # Parameters
    ///
    /// * `value` - Value to serialize for the pending map key.
    ///
    /// # Type Parameters
    ///
    /// * `T` - Map value type accepted by Serde.
    ///
    /// # Errors
    ///
    /// Returns an error if this is not a map, no key is pending, the key is a
    /// duplicate, or value serialization fails.
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
    ///
    /// # Returns
    ///
    /// The completed entries as a JSON object.
    ///
    /// # Errors
    ///
    /// Returns an error if this is not a map or a key has no paired value.
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
    type Error = JsonSerializationError;

    /// Rejects skipped fields in either serde_json private protocol.
    ///
    /// # Parameters
    ///
    /// * `_key` - Struct field name that Serde attempted to skip.
    ///
    /// # Errors
    ///
    /// Returns a private-protocol state error for RawValue or Number compounds.
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
    ///
    /// # Parameters
    ///
    /// * `key` - Static struct field name.
    /// * `value` - Field value to serialize.
    ///
    /// # Type Parameters
    ///
    /// * `T` - Struct field type accepted by Serde.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid compound state, duplicate field, invalid
    /// private protocol field, or nested serialization failure.
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
            Self::RawValue { text, state } => {
                Self::set_private_payload(text, state, JsonPrivateProtocolKind::RawValue, key, value)
            }
            Self::Number { text, state } => {
                Self::set_private_payload(text, state, JsonPrivateProtocolKind::Number, key, value)
            }
            _ => Err(invalid_state(JsonSerializerStateError::UnexpectedCompound)),
        }
    }

    /// Returns the completed struct as a JSON object.
    ///
    /// # Returns
    ///
    /// The accumulated struct fields as a JSON object.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid compound state, pending map key,
    /// incomplete private protocol, or invalid raw/number payload.
    #[inline(always)]
    fn end(self) -> Result<Value, Self::Error> {
        match self {
            Self::Map { values, next_key } => {
                if next_key.is_some() {
                    return Err(invalid_state(JsonSerializerStateError::MapEndedWithPendingKey));
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
    ///
    /// # Parameters
    ///
    /// * `key` - Static struct-variant field name.
    /// * `value` - Field value to serialize.
    ///
    /// # Type Parameters
    ///
    /// * `T` - Struct-variant field type accepted by Serde.
    ///
    /// # Errors
    ///
    /// Returns an error if this is not a struct variant, the field is
    /// duplicated, or nested serialization fails.
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
    ///
    /// # Returns
    ///
    /// An object mapping the variant name to its struct fields.
    ///
    /// # Errors
    ///
    /// Returns an invalid-state error if this is not a struct variant.
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
///
/// # Parameters
///
/// * `reason` - Stable serializer-state violation to retain.
///
/// # Returns
///
/// A privacy-safe serialization error with the specified state reason.
#[inline(always)]
fn invalid_state(reason: JsonSerializerStateError) -> JsonSerializationError {
    JsonSerializationError::new(JsonSerializationErrorKind::InvalidSerializerState { reason })
}

/// Creates the stable error for one private serde_json protocol.
///
/// # Parameters
///
/// * `kind` - Private protocol whose invalid state is reported.
///
/// # Returns
///
/// A stable serialization error describing that protocol's invalid state.
pub(super) fn private_invalid_state(kind: JsonPrivateProtocolKind) -> JsonSerializationError {
    invalid_state(kind.invalid_state())
}

#[cfg(test)]
mod tests {
    use serde::ser::SerializeMap;
    use serde::ser::SerializeSeq;
    use serde::ser::SerializeStruct;
    use serde::ser::SerializeStructVariant;
    use serde::ser::SerializeTuple;
    use serde::ser::SerializeTupleStruct;
    use serde::ser::SerializeTupleVariant;
    use serde_json::Map;
    use serde_json::Value;

    use super::JsonValueCompound;
    use crate::encode::JsonSerializationError;
    use crate::encode::JsonSerializationErrorKind;
    use crate::encode::JsonSerializerStateError;
    use crate::internal::JsonPrivateProtocolKind;
    use crate::internal::JsonPrivateProtocolState;

    fn assert_unexpected_compound(error: JsonSerializationError) {
        assert_eq!(
            error.kind(),
            JsonSerializationErrorKind::InvalidSerializerState {
                reason: JsonSerializerStateError::UnexpectedCompound,
            },
        );
    }

    #[test]
    fn test_sequence_rejects_other_compound_state() {
        assert_unexpected_compound(
            SerializeSeq::serialize_element(&mut JsonValueCompound::map(0), &1_u8)
                .expect_err("a map cannot accept a sequence element"),
        );
        assert_unexpected_compound(
            SerializeSeq::end(JsonValueCompound::map(0)).expect_err("a map cannot end as a sequence"),
        );

        let mut tuple = JsonValueCompound::sequence(1);
        SerializeTuple::serialize_element(&mut tuple, &1_u8).expect("a tuple accepts an element");
        assert_eq!(
            SerializeTuple::end(tuple).expect("a tuple ends as an array"),
            Value::Array(vec![Value::from(1_u8)]),
        );

        let mut tuple_struct = JsonValueCompound::sequence(1);
        SerializeTupleStruct::serialize_field(&mut tuple_struct, &2_u8).expect("a tuple struct accepts a field");
        assert_eq!(
            SerializeTupleStruct::end(tuple_struct).expect("a tuple struct ends as an array"),
            Value::Array(vec![Value::from(2_u8)]),
        );
    }

    #[test]
    fn test_map_rejects_other_compound_state() {
        assert_unexpected_compound(
            SerializeMap::serialize_key(&mut JsonValueCompound::sequence(0), &"key")
                .expect_err("a sequence cannot accept a map key"),
        );
        assert_unexpected_compound(
            SerializeMap::serialize_value(&mut JsonValueCompound::sequence(0), &1_u8)
                .expect_err("a sequence cannot accept a map value"),
        );

        for error in [
            SerializeMap::serialize_value(&mut JsonValueCompound::map(0), &1_u8)
                .expect_err("a map value requires a pending key"),
            SerializeMap::serialize_value(&mut JsonValueCompound::map(0), &Value::from(2_i32))
                .expect_err("a map value requires a pending key"),
            SerializeMap::serialize_value(&mut JsonValueCompound::map(0), &&3_i32)
                .expect_err("a map value requires a pending key"),
            SerializeMap::serialize_value(&mut JsonValueCompound::map(0), &1_i32)
                .expect_err("a map value requires a pending key"),
        ] {
            assert_eq!(
                error.kind(),
                JsonSerializationErrorKind::InvalidSerializerState {
                    reason: JsonSerializerStateError::MapValueWithoutKey,
                },
            );
        }

        let mut pending_map = JsonValueCompound::map(1);
        SerializeMap::serialize_key(&mut pending_map, &"key").expect("a map accepts a key before its value");
        assert_eq!(
            SerializeMap::end(pending_map)
                .expect_err("a map cannot end with a pending key")
                .kind(),
            JsonSerializationErrorKind::InvalidSerializerState {
                reason: JsonSerializerStateError::MapEndedWithPendingKey,
            },
        );
    }

    #[test]
    fn test_tuple_variant_rejects_other_compound_state() {
        assert_unexpected_compound(
            SerializeTupleVariant::serialize_field(&mut JsonValueCompound::sequence(0), &1_u8)
                .expect_err("a sequence cannot accept a tuple-variant field"),
        );
        assert_unexpected_compound(
            SerializeTupleVariant::end(JsonValueCompound::sequence(0))
                .expect_err("a sequence cannot end as a tuple variant"),
        );

        let mut tuple_variant = JsonValueCompound::tuple_variant("Variant", 1);
        SerializeTupleVariant::serialize_field(&mut tuple_variant, &3_u8).expect("a tuple variant accepts a field");
        assert_eq!(
            SerializeTupleVariant::end(tuple_variant).expect("a tuple variant ends as an object"),
            Value::Object(Map::from_iter([(
                String::from("Variant"),
                Value::Array(vec![Value::from(3_u8)]),
            )])),
        );
    }

    #[test]
    fn test_struct_variant_rejects_other_compound_state() {
        assert_unexpected_compound(
            SerializeStructVariant::serialize_field(&mut JsonValueCompound::sequence(0), "field", &1_u8)
                .expect_err("a sequence cannot accept a struct-variant field"),
        );
        assert_unexpected_compound(
            SerializeStructVariant::end(JsonValueCompound::sequence(0))
                .expect_err("a sequence cannot end as a struct variant"),
        );

        let mut struct_variant = JsonValueCompound::struct_variant("Variant", 1);
        SerializeStructVariant::serialize_field(&mut struct_variant, "field", &4_u8)
            .expect("a struct variant accepts a field");
        assert_eq!(
            SerializeStructVariant::end(struct_variant).expect("a struct variant ends as an object"),
            Value::Object(Map::from_iter([(
                String::from("Variant"),
                Value::Object(Map::from_iter([(String::from("field"), Value::from(4_u8))])),
            )])),
        );
    }

    #[test]
    fn test_struct_rejects_other_compound_state() {
        assert_unexpected_compound(
            SerializeStruct::serialize_field(&mut JsonValueCompound::sequence(0), "field", &1_u8)
                .expect_err("a sequence cannot accept a struct field"),
        );
        assert_unexpected_compound(
            SerializeStruct::end(JsonValueCompound::sequence(0)).expect_err("a sequence cannot end as a struct"),
        );

        let mut pending_map = JsonValueCompound::map(1);
        SerializeMap::serialize_key(&mut pending_map, &"key").expect("a map accepts a key before its value");
        assert_eq!(
            SerializeStruct::serialize_field(&mut pending_map, "field", &1_u8)
                .expect_err("a struct field cannot be added while a map key is pending")
                .kind(),
            JsonSerializationErrorKind::InvalidSerializerState {
                reason: JsonSerializerStateError::MapKeyAlreadyPending,
            },
        );

        let mut pending_map = JsonValueCompound::map(1);
        SerializeMap::serialize_key(&mut pending_map, &"key").expect("a map accepts a key before its value");
        assert_eq!(
            SerializeStruct::end(pending_map)
                .expect_err("a struct cannot end while a map key is pending")
                .kind(),
            JsonSerializationErrorKind::InvalidSerializerState {
                reason: JsonSerializerStateError::MapEndedWithPendingKey,
            },
        );
    }

    #[test]
    fn test_private_protocol_skip_field_fails_closed() {
        macro_rules! private_payload_error {
            ($kind:expr, $value:expr) => {{
                let state =
                    JsonPrivateProtocolState::new($kind, 1).expect("one-field private protocol should start pending");
                SerializeStruct::serialize_field(
                    &mut JsonValueCompound::private($kind, state),
                    $kind.field_name(),
                    $value,
                )
                .expect_err("non-text payloads are invalid for private protocols")
            }};
        }

        for kind in [JsonPrivateProtocolKind::RawValue, JsonPrivateProtocolKind::Number] {
            let state =
                JsonPrivateProtocolState::new(kind, 1).expect("one-field private protocol should start pending");
            let error = SerializeStruct::skip_field(&mut JsonValueCompound::private(kind, state), "payload")
                .expect_err("private protocol fields cannot be skipped");
            assert_eq!(
                error.kind(),
                JsonSerializationErrorKind::InvalidSerializerState {
                    reason: kind.invalid_state(),
                },
            );

            let state =
                JsonPrivateProtocolState::new(kind, 1).expect("one-field private protocol should start pending");
            assert_eq!(
                SerializeStruct::end(JsonValueCompound::private(kind, state))
                    .expect_err("a private protocol cannot end before its payload")
                    .kind(),
                JsonSerializationErrorKind::InvalidSerializerState {
                    reason: kind.invalid_state(),
                },
            );

            let state =
                JsonPrivateProtocolState::new(kind, 1).expect("one-field private protocol should start pending");
            assert_eq!(
                SerializeStruct::serialize_field(
                    &mut JsonValueCompound::private(kind, state),
                    kind.field_name(),
                    &1_u8,
                )
                .expect_err("private protocol payload must serialize as text")
                .kind(),
                JsonSerializationErrorKind::InvalidSerializerState {
                    reason: kind.invalid_state(),
                },
            );

            let state =
                JsonPrivateProtocolState::new(kind, 1).expect("one-field private protocol should start pending");
            let mut compound = JsonValueCompound::private(kind, state);
            SerializeStruct::serialize_field(&mut compound, kind.field_name(), &"17")
                .expect("a string is a valid private protocol payload");
            assert_eq!(
                SerializeStruct::end(compound).expect("the numeric text is valid for both protocols"),
                Value::from(17_u8),
            );

            for error in [
                private_payload_error!(kind, &true),
                private_payload_error!(kind, &-17_i32),
                private_payload_error!(kind, &17_u32),
            ] {
                assert_eq!(
                    error.kind(),
                    JsonSerializationErrorKind::InvalidSerializerState {
                        reason: kind.invalid_state(),
                    },
                );
            }
        }
    }
}
