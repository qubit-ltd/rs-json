// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
// =============================================================================
//! Deserializes JSON values while rejecting duplicate object keys.

use std::fmt;

use serde::Deserialize;
use serde::Deserializer;
use serde::de;
use serde::de::DeserializeSeed;
use serde::de::MapAccess;
use serde::de::SeqAccess;
use serde::de::Visitor;
use serde_json::Map;
use serde_json::Number;
use serde_json::Value;

mod strict_json_value_seed;

pub use strict_json_value_seed::StrictJsonValueSeed;

/// A JSON value whose deserialization rejects duplicate object keys.
///
/// This wrapper is useful for document formats where accepting the usual
/// last-key-wins object behavior would make the input ambiguous. It preserves
/// serde_json's arbitrary-precision number representation while recursively
/// validating every object.
///
/// # Examples
///
/// ```
/// use qubit_json::value::StrictJsonValue;
///
/// let value: StrictJsonValue = serde_json::from_str(r#"{"ok":true}"#)?;
/// assert_eq!(value.into_inner()["ok"], true);
/// # Ok::<(), serde_json::Error>(())
/// ```
#[derive(Debug, PartialEq)]
pub struct StrictJsonValue(Value);

impl StrictJsonValue {
    /// Returns the validated JSON value.
    ///
    /// # Returns
    ///
    /// The owned serde_json value constructed during deserialization.
    #[must_use]
    #[inline(always)]
    pub fn into_inner(self) -> Value {
        self.0
    }
}

impl<'de> Deserialize<'de> for StrictJsonValue {
    /// Deserializes JSON recursively while rejecting duplicate object keys.
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        StrictJsonValueSeed::new().deserialize(deserializer)
    }
}

/// Serde visitor that builds one duplicate-key-free JSON value.
struct StrictJsonVisitor;

impl<'de> Visitor<'de> for StrictJsonVisitor {
    type Value = StrictJsonValue;

    /// Describes the expected JSON value shape.
    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON value with unique object keys")
    }

    /// Decodes a JSON boolean.
    fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E> {
        Ok(StrictJsonValue(Value::Bool(value)))
    }

    /// Decodes a signed JSON integer.
    fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E> {
        Ok(StrictJsonValue(Value::Number(value.into())))
    }

    /// Decodes a signed wide JSON integer.
    fn visit_i128<E>(self, value: i128) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Number::from_i128(value)
            .map(|number| StrictJsonValue(Value::Number(number)))
            .ok_or_else(|| de::Error::custom("JSON number out of range"))
    }

    /// Decodes an unsigned JSON integer.
    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E> {
        Ok(StrictJsonValue(Value::Number(value.into())))
    }

    /// Decodes an unsigned wide JSON integer.
    fn visit_u128<E>(self, value: u128) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Number::from_u128(value)
            .map(|number| StrictJsonValue(Value::Number(number)))
            .ok_or_else(|| de::Error::custom("JSON number out of range"))
    }

    /// Decodes a finite JSON floating-point number.
    fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Number::from_f64(value)
            .map(|number| StrictJsonValue(Value::Number(number)))
            .ok_or_else(|| de::Error::custom("not a JSON number"))
    }

    /// Decodes a borrowed JSON string.
    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E> {
        Ok(StrictJsonValue(Value::String(value.to_owned())))
    }

    /// Decodes an owned JSON string.
    fn visit_string<E>(self, value: String) -> Result<Self::Value, E> {
        Ok(StrictJsonValue(Value::String(value)))
    }

    /// Decodes JSON null.
    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(StrictJsonValue(Value::Null))
    }

    /// Decodes an absent optional JSON value as null.
    fn visit_none<E>(self) -> Result<Self::Value, E> {
        Ok(StrictJsonValue(Value::Null))
    }

    /// Decodes a present optional JSON value.
    fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        StrictJsonValue::deserialize(deserializer)
    }

    /// Decodes a JSON array recursively.
    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut values = Vec::new();
        while let Some(value) = sequence.next_element::<StrictJsonValue>()? {
            values.push(value.into_inner());
        }
        Ok(StrictJsonValue(Value::Array(values)))
    }

    /// Decodes a JSON object and rejects repeated keys.
    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let Some(first_key) = map.next_key::<String>()? else {
            return Ok(StrictJsonValue(Value::Object(Map::new())));
        };

        // serde_json uses this private wrapper to preserve arbitrary-precision
        // number text during `deserialize_any`. The exact serde_json version
        // is deliberately locked by this crate's dependency contract.
        if first_key == concat!("$serde_json", "::private::Number") {
            let number_text = map.next_value::<String>()?;
            if map.next_key::<String>()?.is_some() {
                return Err(de::Error::custom("arbitrary-precision number contains extra fields"));
            }
            let number = number_text.parse::<Number>().map_err(de::Error::custom)?;
            return Ok(StrictJsonValue(Value::Number(number)));
        }

        let mut values = Map::new();
        let first_value = map.next_value::<StrictJsonValue>()?;
        values.insert(first_key.clone(), first_value.into_inner());
        while let Some((key, value)) = map.next_entry::<String, StrictJsonValue>()? {
            if values.insert(key.clone(), value.into_inner()).is_some() {
                return Err(de::Error::custom(format!("duplicate JSON object key '{key}'")));
            }
        }
        Ok(StrictJsonValue(Value::Object(values)))
    }
}
