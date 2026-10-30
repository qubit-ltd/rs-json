// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! JSON reference parsing that treats every object key as literal data.

use std::fmt;

use serde::Deserialize;
use serde::Deserializer as _;
use serde::de::DeserializeSeed;
use serde::de::Error;
use serde::de::MapAccess;
use serde::de::SeqAccess;
use serde::de::Visitor;
use serde_json::Map;
use serde_json::Number;
use serde_json::Value;
use serde_json::value::RawValue;

/// Maximum number of nested containers accepted by this bounded fuzz oracle.
const MAX_REFERENCE_CONTAINER_DEPTH: usize = 127;

/// Parses complete JSON bytes without interpreting private Serde object keys.
///
/// Object keys always remain literal data, while numbers follow the active
/// serde_json feature configuration. The bounded oracle accepts at most 127
/// containers; callers should keep fuzz inputs finite and small.
///
/// Returns a serde_json error for invalid syntax, invalid strings or numbers,
/// trailing input, or inputs beyond the reference parser's recursion limit.
pub fn parse_literal_value(input: &[u8]) -> Result<Value, serde_json::Error> {
    let mut parser = serde_json::Deserializer::from_slice(input);
    let value = LiteralValueSeed { container_depth: 0 }.deserialize(&mut parser)?;
    parser.end()?;
    Ok(value)
}

/// Serializes and reparses a value while treating every object key literally.
pub fn roundtrip_literal_value(value: &Value) -> Result<Value, serde_json::Error> {
    let encoded = serde_json::to_vec(value)?;
    parse_literal_value(&encoded)
}

/// Parses one raw value while preserving the distinction between numbers and
/// objects that use serde_json's private protocol-looking keys.
struct LiteralValueSeed {
    container_depth: usize,
}

impl<'de> DeserializeSeed<'de> for LiteralValueSeed {
    type Value = Value;

    fn deserialize<D: serde::Deserializer<'de>>(self, deserializer: D) -> Result<Value, D::Error> {
        let raw = Box::<RawValue>::deserialize(deserializer)?;
        let text = raw.get().trim();
        let first = text.as_bytes().first().copied();
        if first.is_some_and(|byte| matches!(byte, b'-' | b'0'..=b'9')) {
            return serde_json::from_str::<Number>(text)
                .map(Value::Number)
                .map_err(D::Error::custom);
        }
        if matches!(first, Some(b'[' | b'{')) && self.container_depth >= MAX_REFERENCE_CONTAINER_DEPTH {
            return Err(D::Error::custom("literal reference container depth exceeded"));
        }
        let mut parser = serde_json::Deserializer::from_str(text);
        let value = parser
            .deserialize_any(LiteralValueVisitor {
                container_depth: self.container_depth,
            })
            .map_err(D::Error::custom)?;
        parser.end().map_err(D::Error::custom)?;
        Ok(value)
    }
}

/// Builds a Value tree without treating any object key as a protocol token.
struct LiteralValueVisitor {
    container_depth: usize,
}

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
        while let Some(value) = sequence.next_element_seed(LiteralValueSeed {
            container_depth: self.container_depth + 1,
        })? {
            values.push(value);
        }
        Ok(Value::Array(values))
    }

    /// Preserves every object key, including private protocol-looking strings.
    fn visit_map<A: MapAccess<'de>>(self, mut object: A) -> Result<Value, A::Error> {
        let mut values = Map::new();
        while let Some(key) = object.next_key::<String>()? {
            let value = object.next_value_seed(LiteralValueSeed {
                container_depth: self.container_depth + 1,
            })?;
            values.insert(key, value);
        }
        Ok(Value::Object(values))
    }
}
