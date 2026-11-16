// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Protected Serde interoperability for [`crate::value::SafeJsonValue`].
//!
//! Deserialization rejects duplicate object keys and non-finite numbers. Values
//! are assembled through the guarded builder, so an error cannot expose a
//! partially built `SafeJsonValue`. Serialization delegates recursively to the
//! caller's Serde serializer and therefore preserves that serializer's errors.

use std::collections::BTreeSet;
use std::fmt;

use serde::Deserialize;
use serde::Deserializer;
use serde::de::Error as DeError;
use serde::de::MapAccess;
use serde::de::SeqAccess;
use serde::de::Visitor;
use serde_json::Value;

use crate::internal::StrictJsonNumber;
use crate::value::SafeJsonValue;
use crate::value::internal::SafeValueBuilder;

/// Builds a JSON value from Serde events while rejecting duplicate map keys.
///
/// Sequences and objects use `SafeValueBuilder`, which keeps partial children
/// protected and cleans them up if a visitor call fails. Map-shaped events are
/// treated as literal objects: they carry no trustworthy provenance for
/// serde_json private number protocols.
pub(in crate::value) struct ValueVisitor;

impl<'de> Visitor<'de> for ValueVisitor {
    /// The JSON value assembled from the deserializer's event stream.
    type Value = Value;

    /// Describes the JSON value accepted by this visitor in diagnostics.
    ///
    /// # Parameters
    ///
    /// * `formatter` — receives the human-readable expectation.
    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("any JSON value")
    }

    /// Converts a unit event to JSON null.
    ///
    /// # Type Parameters
    ///
    /// * `E` — the deserializer's error type; this event cannot fail.
    fn visit_unit<E>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }
    /// Converts an absent optional value to JSON null.
    ///
    /// # Type Parameters
    ///
    /// * `E` — the deserializer's error type; this event cannot fail.
    fn visit_none<E>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }
    /// Preserves a boolean event as a JSON boolean.
    ///
    /// # Type Parameters
    ///
    /// * `E` — the deserializer's error type; this event cannot fail.
    ///
    /// # Parameters
    ///
    /// * `value` — the boolean supplied by the deserializer.
    fn visit_bool<E>(self, value: bool) -> Result<Value, E> {
        Ok(Value::Bool(value))
    }
    /// Converts a signed 64-bit event to a JSON number.
    ///
    /// # Type Parameters
    ///
    /// * `E` — the deserializer's error type; this event cannot fail.
    ///
    /// # Parameters
    ///
    /// * `value` — the signed integer supplied by the deserializer.
    fn visit_i64<E>(self, value: i64) -> Result<Value, E> {
        Ok(Value::Number(value.into()))
    }
    /// Converts an unsigned 64-bit event to a JSON number.
    ///
    /// # Type Parameters
    ///
    /// * `E` — the deserializer's error type; this event cannot fail.
    ///
    /// # Parameters
    ///
    /// * `value` — the unsigned integer supplied by the deserializer.
    fn visit_u64<E>(self, value: u64) -> Result<Value, E> {
        Ok(Value::Number(value.into()))
    }
    /// Converts an `i128` event when it fits the JSON integer range.
    ///
    /// # Type Parameters
    ///
    /// * `E` — the deserializer error type used when the integer is out of
    ///   range.
    ///
    /// # Parameters
    ///
    /// * `value` — the signed integer supplied by the deserializer.
    ///
    /// # Errors
    ///
    /// Returns the deserializer's error when the integer is outside the JSON
    /// range.
    fn visit_i128<E: DeError>(self, value: i128) -> Result<Value, E> {
        StrictJsonNumber::from_i128(value)
            .map(Value::Number)
            .map_err(E::custom)
    }
    /// Converts a `u128` event when it fits the JSON integer range.
    ///
    /// # Type Parameters
    ///
    /// * `E` — the deserializer error type used when the integer is out of
    ///   range.
    ///
    /// # Parameters
    ///
    /// * `value` — the unsigned integer supplied by the deserializer.
    ///
    /// # Errors
    ///
    /// Returns the deserializer's error when the integer is outside the JSON
    /// range.
    fn visit_u128<E: DeError>(self, value: u128) -> Result<Value, E> {
        StrictJsonNumber::from_u128(value)
            .map(Value::Number)
            .map_err(E::custom)
    }
    /// Deserializes an optional payload while retaining stack protection.
    ///
    /// # Type Parameters
    ///
    /// * `D` — the deserializer that supplies the optional payload and its
    ///   error type.
    ///
    /// # Parameters
    ///
    /// * `deserializer` — the source of the wrapped value.
    ///
    /// # Errors
    ///
    /// Returns an error from deserializing the wrapped value.
    fn visit_some<D: Deserializer<'de>>(self, deserializer: D) -> Result<Value, D::Error> {
        SafeJsonValue::deserialize(deserializer).map(SafeJsonValue::into_value)
    }
    /// Deserializes a newtype payload while retaining stack protection.
    ///
    /// # Type Parameters
    ///
    /// * `D` — the deserializer that supplies the wrapped payload and its error
    ///   type.
    ///
    /// # Parameters
    ///
    /// * `deserializer` — the source of the newtype payload.
    ///
    /// # Errors
    ///
    /// Returns an error from deserializing the wrapped value.
    fn visit_newtype_struct<D: Deserializer<'de>>(
        self,
        deserializer: D,
    ) -> Result<Value, D::Error> {
        SafeJsonValue::deserialize(deserializer).map(SafeJsonValue::into_value)
    }
    /// Converts a finite floating-point event to a JSON number.
    ///
    /// # Type Parameters
    ///
    /// * `E` — the deserializer error type used for non-finite values.
    ///
    /// # Parameters
    ///
    /// * `value` — the floating-point value supplied by the deserializer.
    ///
    /// # Errors
    ///
    /// Returns the deserializer's error when the value is not finite.
    fn visit_f64<E: serde::de::Error>(self, value: f64) -> Result<Value, E> {
        serde_json::Number::from_f64(value)
            .map(Value::Number)
            .ok_or_else(|| E::custom("non-finite float"))
    }
    /// Copies a borrowed string event into an owned JSON string.
    ///
    /// # Type Parameters
    ///
    /// * `E` — the deserializer's error type; this event cannot fail.
    ///
    /// # Parameters
    ///
    /// * `value` — the string borrowed from the deserializer.
    fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<Value, E> {
        Ok(Value::String(value.to_owned()))
    }
    /// Takes ownership of a string event as a JSON string.
    ///
    /// # Type Parameters
    ///
    /// * `E` — the deserializer's error type; this event cannot fail.
    ///
    /// # Parameters
    ///
    /// * `value` — the owned string supplied by the deserializer.
    fn visit_string<E>(self, value: String) -> Result<Value, E> {
        Ok(Value::String(value))
    }

    /// Collects sequence elements through the guarded builder.
    ///
    /// # Type Parameters
    ///
    /// * `A` — the sequence access implementation and its error type.
    ///
    /// # Parameters
    ///
    /// * `seq` — the deserializer's sequence cursor, consumed until exhausted.
    ///
    /// # Errors
    ///
    /// Returns an error when reading any sequence element fails.
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Value, A::Error> {
        let mut values = SafeValueBuilder::array();
        while let Some(value) = seq.next_element::<SafeJsonValue>()? {
            values.push(value);
        }
        Ok(values.finish())
    }

    /// Collects map entries and rejects duplicate keys before building an
    /// object.
    ///
    /// # Type Parameters
    ///
    /// * `A` — the map access implementation and its error type.
    ///
    /// # Parameters
    ///
    /// * `map` — the deserializer's map cursor, consumed until exhausted.
    ///
    /// # Errors
    ///
    /// Returns an error when reading an entry fails or a key is duplicated.
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Value, A::Error> {
        let mut keys = BTreeSet::new();
        let mut values: Vec<(String, SafeJsonValue)> = Vec::new();
        while let Some(key) = map.next_key::<String>()? {
            if !keys.insert(key.clone()) {
                return Err(serde::de::Error::custom("duplicate JSON object key"));
            }
            let value = map.next_value::<SafeJsonValue>()?;
            values.push((key, value));
        }
        let mut object = SafeValueBuilder::object();
        for (key, value) in values {
            object.insert(key, value);
        }
        Ok(object.finish())
    }
}
