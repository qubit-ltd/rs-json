// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Protected Serde interoperability for [`super::SafeJsonValue`].

use std::fmt;

use serde::Deserialize;
use serde::Deserializer;
use serde::Serialize;
use serde::Serializer;
use serde::de::MapAccess;
use serde::de::SeqAccess;
use serde::de::Visitor;
use serde::ser::SerializeMap;
use serde::ser::SerializeSeq;
use serde_json::Value;

use super::SafeJsonValue;
use super::safe_json_value::SafeValueBuilder;

impl Serialize for SafeJsonValue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize_value(self.as_value(), serializer)
    }
}

fn serialize_value<S>(value: &Value, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    stacker::maybe_grow(32 * 1024, 1024 * 1024, || match value {
        Value::Null => serializer.serialize_unit(),
        Value::Bool(value) => serializer.serialize_bool(*value),
        Value::Number(value) => value.serialize(serializer),
        Value::String(value) => serializer.serialize_str(value),
        Value::Array(values) => {
            let mut seq = serializer.serialize_seq(Some(values.len()))?;
            for child in values {
                seq.serialize_element(&Nested(child))?;
            }
            seq.end()
        }
        Value::Object(values) => {
            let mut map = serializer.serialize_map(Some(values.len()))?;
            for (key, child) in values {
                map.serialize_entry(key, &Nested(child))?;
            }
            map.end()
        }
    })
}

struct Nested<'a>(&'a Value);
impl Serialize for Nested<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize_value(self.0, serializer)
    }
}

impl<'de> Deserialize<'de> for SafeJsonValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        stacker::maybe_grow(32 * 1024, 1024 * 1024, || {
            deserializer.deserialize_any(ValueVisitor).map(SafeJsonValue::new)
        })
    }
}

struct ValueVisitor;

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
        let mut values: Vec<(String, SafeJsonValue)> = Vec::new();
        while let Some(key) = map.next_key::<String>()? {
            if values.iter().any(|(existing, _)| existing == &key) {
                return Err(serde::de::Error::custom("duplicate object key"));
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
