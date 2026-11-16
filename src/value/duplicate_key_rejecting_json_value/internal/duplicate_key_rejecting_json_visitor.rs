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
use crate::internal::StrictJsonNumber;

/// Builds one duplicate-key-free JSON value from Serde events.
pub(in crate::value::duplicate_key_rejecting_json_value) struct DuplicateKeyRejectingJsonVisitor;

impl<'de> Visitor<'de> for DuplicateKeyRejectingJsonVisitor {
    /// JSON value wrapper produced after each Serde value is validated.
    type Value = DuplicateKeyRejectingJsonValue;

    /// Describes the expected JSON value shape.
    ///
    /// # Parameters
    ///
    /// * `formatter` - Receives the human-readable description.
    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON value with unique object keys")
    }

    /// Decodes a JSON boolean.
    ///
    /// # Type Parameters
    ///
    /// * `E` - Error type used by the deserializer.
    ///
    /// # Parameters
    ///
    /// * `value` - Boolean event to convert.
    fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E> {
        Ok(DuplicateKeyRejectingJsonValue(Value::Bool(value)))
    }

    /// Decodes a signed JSON integer.
    ///
    /// # Type Parameters
    ///
    /// * `E` - Error type used by the deserializer.
    ///
    /// # Parameters
    ///
    /// * `value` - Signed integer event to convert.
    fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E> {
        Ok(DuplicateKeyRejectingJsonValue(Value::Number(value.into())))
    }

    /// Decodes a signed wide JSON integer within serde_json's value range.
    ///
    /// # Type Parameters
    ///
    /// * `E` - Error type used by the deserializer.
    ///
    /// # Parameters
    ///
    /// * `value` - Signed integer event to convert.
    fn visit_i128<E>(self, value: i128) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        StrictJsonNumber::from_i128(value)
            .map_err(de::Error::custom)
            .map(|number| DuplicateKeyRejectingJsonValue(Value::Number(number)))
    }

    /// Decodes an unsigned JSON integer.
    ///
    /// # Type Parameters
    ///
    /// * `E` - Error type used by the deserializer.
    ///
    /// # Parameters
    ///
    /// * `value` - Unsigned integer event to convert.
    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E> {
        Ok(DuplicateKeyRejectingJsonValue(Value::Number(value.into())))
    }

    /// Decodes an unsigned wide JSON integer within serde_json's value range.
    ///
    /// # Type Parameters
    ///
    /// * `E` - Error type used by the deserializer.
    ///
    /// # Parameters
    ///
    /// * `value` - Unsigned integer event to convert.
    fn visit_u128<E>(self, value: u128) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        StrictJsonNumber::from_u128(value)
            .map_err(de::Error::custom)
            .map(|number| DuplicateKeyRejectingJsonValue(Value::Number(number)))
    }

    /// Decodes a finite JSON floating-point number.
    ///
    /// # Type Parameters
    ///
    /// * `E` - Error type used by the deserializer.
    ///
    /// # Parameters
    ///
    /// * `value` - Floating-point event to convert.
    fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Number::from_f64(value)
            .map(|number| DuplicateKeyRejectingJsonValue(Value::Number(number)))
            .ok_or_else(|| de::Error::custom("not a JSON number"))
    }

    /// Decodes a borrowed JSON string into owned storage.
    ///
    /// # Type Parameters
    ///
    /// * `E` - Error type used by the deserializer.
    ///
    /// # Parameters
    ///
    /// * `value` - Borrowed string event to copy.
    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E> {
        Ok(DuplicateKeyRejectingJsonValue(Value::String(
            value.to_owned(),
        )))
    }

    /// Decodes an owned JSON string without another allocation.
    ///
    /// # Type Parameters
    ///
    /// * `E` - Error type used by the deserializer.
    ///
    /// # Parameters
    ///
    /// * `value` - Owned string event to retain.
    fn visit_string<E>(self, value: String) -> Result<Self::Value, E> {
        Ok(DuplicateKeyRejectingJsonValue(Value::String(value)))
    }

    /// Decodes JSON null.
    ///
    /// # Type Parameters
    ///
    /// * `E` - Error type used by the deserializer.
    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(DuplicateKeyRejectingJsonValue(Value::Null))
    }

    /// Decodes an absent optional JSON value as null.
    ///
    /// # Type Parameters
    ///
    /// * `E` - Error type used by the deserializer.
    fn visit_none<E>(self) -> Result<Self::Value, E> {
        Ok(DuplicateKeyRejectingJsonValue(Value::Null))
    }

    /// Decodes a present optional JSON value through the strict visitor.
    ///
    /// # Type Parameters
    ///
    /// * `D` - Deserializer that supplies the optional value.
    ///
    /// # Parameters
    ///
    /// * `deserializer` - Source of the present value.
    fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        DuplicateKeyRejectingJsonValue::deserialize(deserializer)
    }

    /// Decodes a JSON array recursively.
    ///
    /// # Type Parameters
    ///
    /// * `A` - Sequence access implementation supplied by Serde.
    ///
    /// # Parameters
    ///
    /// * `sequence` - Access to the array elements.
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
    ///
    /// # Type Parameters
    ///
    /// * `A` - Map access implementation supplied by Serde.
    ///
    /// # Parameters
    ///
    /// * `map` - Access to the object entries.
    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut values = Map::new();
        while let Some(key) = map.next_key::<String>()? {
            if values.contains_key(&key) {
                return Err(de::Error::custom("duplicate JSON object key"));
            }
            let value = map.next_value::<DuplicateKeyRejectingJsonValue>()?;
            values.insert(key, value.into_inner());
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

    use super::DuplicateKeyRejectingJsonVisitor;

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
        assert!(
            DuplicateKeyRejectingJsonVisitor
                .visit_i128::<Error>(i128::MIN)
                .is_err()
        );
        assert_eq!(
            DuplicateKeyRejectingJsonVisitor
                .visit_u128::<Error>(u64::MAX.into())
                .expect("u64 maximum must decode")
                .into_inner(),
            Value::from(u64::MAX),
        );
        assert!(
            DuplicateKeyRejectingJsonVisitor
                .visit_u128::<Error>(u128::MAX)
                .is_err()
        );
    }
}
