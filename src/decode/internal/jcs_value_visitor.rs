// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Binary64 JSON value visitor used by [`super::jcs_value_seed::JcsValueSeed`].

use std::cell::Cell;
use std::fmt;

use serde::de::Error;
use serde::de::MapAccess;
use serde::de::SeqAccess;
use serde::de::Visitor;

use super::jcs_value_seed::JcsValueSeed;

/// Builds a serde_json value while preserving JCS duplicate-key semantics.
pub(in crate::decode) struct JcsValueVisitor<'a> {
    pub(in crate::decode) duplicate_key: &'a Cell<bool>,
}

fn number<E: Error>(value: f64) -> Result<serde_json::Value, E> {
    serde_json::Number::from_f64(value)
        .map(serde_json::Value::Number)
        .ok_or_else(|| E::custom("JCS number is outside finite binary64 range"))
}

impl<'de> Visitor<'de> for JcsValueVisitor<'_> {
    type Value = serde_json::Value;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON value")
    }

    fn visit_bool<E: Error>(self, value: bool) -> Result<Self::Value, E> {
        Ok(value.into())
    }
    fn visit_unit<E: Error>(self) -> Result<Self::Value, E> {
        Ok(serde_json::Value::Null)
    }
    fn visit_str<E: Error>(self, value: &str) -> Result<Self::Value, E> {
        Ok(value.into())
    }
    fn visit_string<E: Error>(self, value: String) -> Result<Self::Value, E> {
        Ok(value.into())
    }
    fn visit_i64<E: Error>(self, value: i64) -> Result<Self::Value, E> {
        number(value as f64)
    }
    fn visit_u64<E: Error>(self, value: u64) -> Result<Self::Value, E> {
        number(value as f64)
    }
    fn visit_f64<E: Error>(self, value: f64) -> Result<Self::Value, E> {
        number(value)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Self::Value, A::Error> {
        let mut values = Vec::new();
        while let Some(value) = sequence.next_element_seed(JcsValueSeed {
            duplicate_key: self.duplicate_key,
        })? {
            values.push(value);
        }
        Ok(serde_json::Value::Array(values))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        let mut values = serde_json::Map::new();
        while let Some(key) = map.next_key::<String>()? {
            if values.contains_key(&key) {
                self.duplicate_key.set(true);
                return Err(A::Error::custom("duplicate JSON object key"));
            }
            let value = map.next_value_seed(JcsValueSeed {
                duplicate_key: self.duplicate_key,
            })?;
            values.insert(key, value);
        }
        Ok(serde_json::Value::Object(values))
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use serde::de::Visitor;

    use super::JcsValueVisitor;

    #[test]
    fn test_visitor_converts_integer_callbacks_to_binary64() {
        let duplicate_key = Cell::new(false);
        let visitor = JcsValueVisitor {
            duplicate_key: &duplicate_key,
        };
        assert_eq!(
            visitor.visit_i64::<serde::de::value::Error>(-7).unwrap().as_f64(),
            Some(-7.0)
        );

        let duplicate_key = Cell::new(false);
        let visitor = JcsValueVisitor {
            duplicate_key: &duplicate_key,
        };
        assert_eq!(
            visitor.visit_u64::<serde::de::value::Error>(7).unwrap().as_f64(),
            Some(7.0)
        );
    }

    #[test]
    fn test_visitor_accepts_owned_string_callback() {
        let duplicate_key = Cell::new(false);
        let visitor = JcsValueVisitor {
            duplicate_key: &duplicate_key,
        };
        assert_eq!(
            visitor
                .visit_string::<serde::de::value::Error>(String::from("owned"))
                .unwrap(),
            serde_json::Value::String(String::from("owned")),
        );
    }

    #[test]
    fn test_visitor_rejects_non_finite_float_callback() {
        let duplicate_key = Cell::new(false);
        let visitor = JcsValueVisitor {
            duplicate_key: &duplicate_key,
        };
        assert!(visitor.visit_f64::<serde::de::value::Error>(f64::INFINITY).is_err());
    }
}
