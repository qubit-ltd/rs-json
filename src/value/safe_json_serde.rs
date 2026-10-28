// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Protected Serde interoperability for [`super::SafeJsonValue`].
//!
//! Deserialization rejects duplicate object keys and non-finite numbers. Values
//! are assembled through the guarded builder, so an error cannot expose a
//! partially built `SafeJsonValue`. Serialization delegates recursively to the
//! caller's Serde serializer and therefore preserves that serializer's errors.

use serde::Deserialize;
use serde::Deserializer;
use serde::Serialize;
use serde::Serializer;
use serde::ser::SerializeMap;
use serde::ser::SerializeSeq;
use serde_json::Value;

use super::SafeJsonValue;
use super::internal::Nested;
use super::internal::ValueVisitor;

impl Serialize for SafeJsonValue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize_value(self.as_value(), serializer)
    }
}

/// Serializes a JSON value recursively while growing the stack for this bridge.
///
/// `value` is borrowed for the duration of the serializer calls. The stacker
/// guard covers this bridge’s recursive traversal, but cannot control recursive
/// work performed internally by the caller’s serializer. Child and serializer
/// errors propagate unchanged.
///
/// # Parameters
///
/// * `value` - JSON subtree to serialize without taking ownership.
/// * `serializer` - Destination Serde serializer.
///
/// # Returns
///
/// The serializer’s successful output.
///
/// # Errors
///
/// Returns the serializer’s error from a scalar, child, or container operation.
pub(in crate::value) fn serialize_value<S>(value: &Value, serializer: S) -> Result<S::Ok, S::Error>
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
