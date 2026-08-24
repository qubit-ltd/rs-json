// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Serde visitor that rejects duplicate keys while building a JSON value.

use std::fmt;

use serde::Deserialize;
use serde::Deserializer;
use serde::de;
use serde::de::MapAccess;
use serde::de::SeqAccess;
use serde::de::Visitor;
use serde_json::Map;
use serde_json::Number;
use serde_json::Value;

use super::super::StrictJsonValue;

/// Builds one duplicate-key-free JSON value from Serde events.
pub(in crate::value::strict_json_value) struct StrictJsonVisitor;

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

    /// Decodes a signed wide JSON integer within serde_json's value range.
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

    /// Decodes an unsigned wide JSON integer within serde_json's value range.
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

    /// Decodes a borrowed JSON string into owned storage.
    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E> {
        Ok(StrictJsonValue(Value::String(value.to_owned())))
    }

    /// Decodes an owned JSON string without another allocation.
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

    /// Decodes a present optional JSON value through the strict visitor.
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

#[cfg(test)]
mod tests {
    use serde::de::Visitor;
    use serde::de::value::BoolDeserializer;
    use serde::de::value::Error;
    use serde_json::Value;
    use serde_json::from_str;

    use super::StrictJsonVisitor;
    use crate::value::StrictJsonValue;

    /// Covers the private visitor's scalar Serde entry points.
    #[test]
    fn test_strict_json_visitor_covers_scalar_shapes() {
        assert_eq!(
            StrictJsonVisitor
                .visit_bool::<Error>(true)
                .expect("boolean must decode")
                .into_inner(),
            Value::Bool(true),
        );
        assert_eq!(
            StrictJsonVisitor
                .visit_i64::<Error>(-1)
                .expect("signed integer must decode")
                .into_inner(),
            Value::from(-1),
        );
        assert_eq!(
            StrictJsonVisitor
                .visit_u64::<Error>(1)
                .expect("unsigned integer must decode")
                .into_inner(),
            Value::from(1_u64),
        );
        assert_eq!(
            StrictJsonVisitor
                .visit_f64::<Error>(1.5)
                .expect("finite float must decode")
                .into_inner(),
            Value::from(1.5),
        );
        assert!(StrictJsonVisitor.visit_f64::<Error>(f64::INFINITY).is_err());
        assert_eq!(
            StrictJsonVisitor
                .visit_str::<Error>("text")
                .expect("borrowed string must decode")
                .into_inner(),
            Value::from("text"),
        );
        assert_eq!(
            StrictJsonVisitor
                .visit_string::<Error>(String::from("owned"))
                .expect("owned string must decode")
                .into_inner(),
            Value::from("owned"),
        );
        assert_eq!(
            StrictJsonVisitor
                .visit_unit::<Error>()
                .expect("unit must decode")
                .into_inner(),
            Value::Null,
        );
        assert_eq!(
            StrictJsonVisitor
                .visit_none::<Error>()
                .expect("none must decode")
                .into_inner(),
            Value::Null,
        );
        assert_eq!(
            StrictJsonVisitor
                .visit_some(BoolDeserializer::<Error>::new(true))
                .expect("some must decode")
                .into_inner(),
            Value::Bool(true),
        );
    }

    /// Covers accepted and rejected wide integer boundaries.
    #[test]
    fn test_strict_json_visitor_enforces_wide_integer_range() {
        assert_eq!(
            StrictJsonVisitor
                .visit_i128::<Error>(i64::MIN.into())
                .expect("i64 minimum must decode")
                .into_inner(),
            Value::from(i64::MIN),
        );
        assert!(StrictJsonVisitor.visit_i128::<Error>(i128::MIN).is_err());
        assert_eq!(
            StrictJsonVisitor
                .visit_u128::<Error>(u64::MAX.into())
                .expect("u64 maximum must decode")
                .into_inner(),
            Value::from(u64::MAX),
        );
        assert!(StrictJsonVisitor.visit_u128::<Error>(u128::MAX).is_err());
    }

    /// Verifies invalid input still reports serde_json's expected-value error.
    #[test]
    fn test_strict_json_visitor_describes_expected_value() {
        assert_eq!(
            from_str::<StrictJsonValue>("invalid")
                .expect_err("invalid JSON must fail")
                .to_string(),
            "expected value at line 1 column 1",
        );
    }
}
