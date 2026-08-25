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

use super::super::DuplicateKeyRejectingJsonValue;

/// Builds one duplicate-key-free JSON value from Serde events.
pub(in crate::value::duplicate_key_rejecting_json_value) struct DuplicateKeyRejectingJsonVisitor;

impl<'de> Visitor<'de> for DuplicateKeyRejectingJsonVisitor {
    type Value = DuplicateKeyRejectingJsonValue;

    /// Describes the expected JSON value shape.
    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON value with unique object keys")
    }

    /// Decodes a JSON boolean.
    fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E> {
        Ok(DuplicateKeyRejectingJsonValue(Value::Bool(value)))
    }

    /// Decodes a signed JSON integer.
    fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E> {
        Ok(DuplicateKeyRejectingJsonValue(Value::Number(value.into())))
    }

    /// Decodes a signed wide JSON integer within serde_json's value range.
    fn visit_i128<E>(self, value: i128) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Number::from_i128(value)
            .map(|number| DuplicateKeyRejectingJsonValue(Value::Number(number)))
            .ok_or_else(|| de::Error::custom("JSON number out of range"))
    }

    /// Decodes an unsigned JSON integer.
    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E> {
        Ok(DuplicateKeyRejectingJsonValue(Value::Number(value.into())))
    }

    /// Decodes an unsigned wide JSON integer within serde_json's value range.
    fn visit_u128<E>(self, value: u128) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Number::from_u128(value)
            .map(|number| DuplicateKeyRejectingJsonValue(Value::Number(number)))
            .ok_or_else(|| de::Error::custom("JSON number out of range"))
    }

    /// Decodes a finite JSON floating-point number.
    fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Number::from_f64(value)
            .map(|number| DuplicateKeyRejectingJsonValue(Value::Number(number)))
            .ok_or_else(|| de::Error::custom("not a JSON number"))
    }

    /// Decodes a borrowed JSON string into owned storage.
    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E> {
        Ok(DuplicateKeyRejectingJsonValue(Value::String(value.to_owned())))
    }

    /// Decodes an owned JSON string without another allocation.
    fn visit_string<E>(self, value: String) -> Result<Self::Value, E> {
        Ok(DuplicateKeyRejectingJsonValue(Value::String(value)))
    }

    /// Decodes JSON null.
    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(DuplicateKeyRejectingJsonValue(Value::Null))
    }

    /// Decodes an absent optional JSON value as null.
    fn visit_none<E>(self) -> Result<Self::Value, E> {
        Ok(DuplicateKeyRejectingJsonValue(Value::Null))
    }

    /// Decodes a present optional JSON value through the strict visitor.
    fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        DuplicateKeyRejectingJsonValue::deserialize(deserializer)
    }

    /// Decodes a JSON array recursively.
    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut values = Vec::new();
        while let Some(value) = sequence.next_element::<DuplicateKeyRejectingJsonValue>()? {
            values.push(value.into_inner());
        }
        Ok(DuplicateKeyRejectingJsonValue(Value::Array(values)))
    }

    /// Decodes a JSON object and rejects repeated keys.
    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let Some(first_key) = map.next_key::<String>()? else {
            return Ok(DuplicateKeyRejectingJsonValue(Value::Object(Map::new())));
        };

        let mut values = Map::new();
        let first_value = map.next_value::<DuplicateKeyRejectingJsonValue>()?;
        values.insert(first_key.clone(), first_value.into_inner());
        while let Some((key, value)) = map.next_entry::<String, DuplicateKeyRejectingJsonValue>()? {
            if values.insert(key.clone(), value.into_inner()).is_some() {
                return Err(de::Error::custom(format!("duplicate JSON object key '{key}'")));
            }
        }
        Ok(DuplicateKeyRejectingJsonValue(Value::Object(values)))
    }
}

#[cfg(test)]
mod tests {
    use serde::de::Visitor;
    use serde::de::value::BoolDeserializer;
    use serde::de::value::Error;
    use serde_json::Value;
    use serde_json::from_str;

    use super::DuplicateKeyRejectingJsonVisitor;
    use crate::value::DuplicateKeyRejectingJsonValue;

    /// Covers the private visitor's scalar Serde entry points.
    #[test]
    fn test_duplicate_key_rejecting_json_visitor_covers_scalar_shapes() {
        assert_eq!(
            DuplicateKeyRejectingJsonVisitor
                .visit_bool::<Error>(true)
                .expect("boolean must decode")
                .into_inner(),
            Value::Bool(true),
        );
        assert_eq!(
            DuplicateKeyRejectingJsonVisitor
                .visit_i64::<Error>(-1)
                .expect("signed integer must decode")
                .into_inner(),
            Value::from(-1),
        );
        assert_eq!(
            DuplicateKeyRejectingJsonVisitor
                .visit_u64::<Error>(1)
                .expect("unsigned integer must decode")
                .into_inner(),
            Value::from(1_u64),
        );
        assert_eq!(
            DuplicateKeyRejectingJsonVisitor
                .visit_f64::<Error>(1.5)
                .expect("finite float must decode")
                .into_inner(),
            Value::from(1.5),
        );
        assert!(
            DuplicateKeyRejectingJsonVisitor
                .visit_f64::<Error>(f64::INFINITY)
                .is_err()
        );
        assert_eq!(
            DuplicateKeyRejectingJsonVisitor
                .visit_str::<Error>("text")
                .expect("borrowed string must decode")
                .into_inner(),
            Value::from("text"),
        );
        assert_eq!(
            DuplicateKeyRejectingJsonVisitor
                .visit_string::<Error>(String::from("owned"))
                .expect("owned string must decode")
                .into_inner(),
            Value::from("owned"),
        );
        assert_eq!(
            DuplicateKeyRejectingJsonVisitor
                .visit_unit::<Error>()
                .expect("unit must decode")
                .into_inner(),
            Value::Null,
        );
        assert_eq!(
            DuplicateKeyRejectingJsonVisitor
                .visit_none::<Error>()
                .expect("none must decode")
                .into_inner(),
            Value::Null,
        );
        assert_eq!(
            DuplicateKeyRejectingJsonVisitor
                .visit_some(BoolDeserializer::<Error>::new(true))
                .expect("some must decode")
                .into_inner(),
            Value::Bool(true),
        );
    }

    /// Covers accepted and rejected wide integer boundaries.
    #[test]
    fn test_duplicate_key_rejecting_json_visitor_enforces_wide_integer_range() {
        assert_eq!(
            DuplicateKeyRejectingJsonVisitor
                .visit_i128::<Error>(i64::MIN.into())
                .expect("i64 minimum must decode")
                .into_inner(),
            Value::from(i64::MIN),
        );
        assert!(DuplicateKeyRejectingJsonVisitor.visit_i128::<Error>(i128::MIN).is_err());
        assert_eq!(
            DuplicateKeyRejectingJsonVisitor
                .visit_u128::<Error>(u64::MAX.into())
                .expect("u64 maximum must decode")
                .into_inner(),
            Value::from(u64::MAX),
        );
        assert!(DuplicateKeyRejectingJsonVisitor.visit_u128::<Error>(u128::MAX).is_err());
    }

    /// Verifies invalid input still reports serde_json's expected-value error.
    #[test]
    fn test_duplicate_key_rejecting_json_visitor_describes_expected_value() {
        assert_eq!(
            from_str::<DuplicateKeyRejectingJsonValue>("invalid")
                .expect_err("invalid JSON must fail")
                .to_string(),
            "expected value at line 1 column 1",
        );
    }
}
