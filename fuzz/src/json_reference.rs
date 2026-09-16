// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
// =============================================================================
//! JSON reference parsing that treats every object key as literal data.

use std::fmt;

use serde::Deserialize;
use serde::Deserializer;
use serde::de::Error;
use serde::de::MapAccess;
use serde::de::SeqAccess;
use serde::de::Visitor;
use serde_json::Map;
use serde_json::Number;
use serde_json::Value;

/// Parses complete JSON bytes without interpreting private Serde object keys.
///
/// Returns a serde_json error for invalid syntax, invalid strings or numbers,
/// trailing input, or inputs beyond the reference parser's recursion limit.
pub fn parse_literal_value(input: &[u8]) -> Result<Value, serde_json::Error> {
    let LiteralValue(value) = serde_json::from_slice(input)?;
    Ok(value)
}

/// Recursively requests ordinary JSON values from serde_json's deserializer.
struct LiteralValue(Value);

impl<'de> Deserialize<'de> for LiteralValue {
    /// Uses the same syntax parser while bypassing Value's private map
    /// protocol.
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(LiteralValueVisitor).map(Self)
    }
}

/// Builds a Value tree without treating any object key as a protocol token.
struct LiteralValueVisitor;

impl<'de> Visitor<'de> for LiteralValueVisitor {
    type Value = Value;

    /// Describes the accepted wire value in parser errors.
    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON value")
    }

    /// Preserves JSON null.
    fn visit_unit<E: Error>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }

    /// Preserves booleans.
    fn visit_bool<E: Error>(self, value: bool) -> Result<Value, E> {
        Ok(Value::Bool(value))
    }

    /// Preserves signed integers.
    fn visit_i64<E: Error>(self, value: i64) -> Result<Value, E> {
        Ok(Value::from(value))
    }

    /// Preserves unsigned integers.
    fn visit_u64<E: Error>(self, value: u64) -> Result<Value, E> {
        Ok(Value::from(value))
    }

    /// Preserves finite floating-point values and rejects non-JSON numbers.
    fn visit_f64<E: Error>(self, value: f64) -> Result<Value, E> {
        Number::from_f64(value)
            .map(Value::Number)
            .ok_or_else(|| E::custom("non-finite JSON number"))
    }

    /// Copies strings whose escapes and Unicode were checked by the parser.
    fn visit_str<E: Error>(self, value: &str) -> Result<Value, E> {
        Ok(Value::String(value.to_owned()))
    }

    /// Recursively parses array elements using the literal-key visitor.
    fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Value, A::Error> {
        let mut values = Vec::new();
        while let Some(LiteralValue(value)) = sequence.next_element()? {
            values.push(value);
        }
        Ok(Value::Array(values))
    }

    /// Preserves every object key, including private protocol-looking strings.
    fn visit_map<A: MapAccess<'de>>(self, mut object: A) -> Result<Value, A::Error> {
        let mut values = Map::new();
        while let Some((key, LiteralValue(value))) = object.next_entry::<String, LiteralValue>()? {
            values.insert(key, value);
        }
        Ok(Value::Object(values))
    }
}
