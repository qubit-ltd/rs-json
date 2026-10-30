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
    type Value = Value;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("any JSON value")
    }

    fn visit_unit<E>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }
    fn visit_none<E>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }
    fn visit_bool<E>(self, value: bool) -> Result<Value, E> {
        Ok(Value::Bool(value))
    }
    fn visit_i64<E>(self, value: i64) -> Result<Value, E> {
        Ok(Value::Number(value.into()))
    }
    fn visit_u64<E>(self, value: u64) -> Result<Value, E> {
        Ok(Value::Number(value.into()))
    }
    /// Accepts an `i128` event only when its value fits the JSON integer range.
    fn visit_i128<E: DeError>(self, value: i128) -> Result<Value, E> {
        StrictJsonNumber::from_i128(value).map(Value::Number).map_err(E::custom)
    }
    /// Accepts a `u128` event only when its value fits the JSON integer range.
    fn visit_u128<E: DeError>(self, value: u128) -> Result<Value, E> {
        StrictJsonNumber::from_u128(value).map(Value::Number).map_err(E::custom)
    }
    /// Transparently deserializes the payload while retaining stack protection.
    fn visit_some<D: Deserializer<'de>>(self, deserializer: D) -> Result<Value, D::Error> {
        SafeJsonValue::deserialize(deserializer).map(SafeJsonValue::into_value)
    }
    /// Transparently deserializes the payload while retaining stack protection.
    fn visit_newtype_struct<D: Deserializer<'de>>(self, deserializer: D) -> Result<Value, D::Error> {
        SafeJsonValue::deserialize(deserializer).map(SafeJsonValue::into_value)
    }
    fn visit_f64<E: serde::de::Error>(self, value: f64) -> Result<Value, E> {
        serde_json::Number::from_f64(value)
            .map(Value::Number)
            .ok_or_else(|| E::custom("non-finite float"))
    }
    fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<Value, E> {
        Ok(Value::String(value.to_owned()))
    }
    fn visit_string<E>(self, value: String) -> Result<Value, E> {
        Ok(Value::String(value))
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Value, A::Error> {
        let mut values = SafeValueBuilder::array();
        while let Some(value) = seq.next_element::<SafeJsonValue>()? {
            values.push(value);
        }
        Ok(values.finish())
    }

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
