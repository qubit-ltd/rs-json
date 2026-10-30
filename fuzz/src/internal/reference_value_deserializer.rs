// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use serde::Deserialize;
use serde::Deserializer as _;
use serde::de::Error as _;
use serde::de::Visitor;

/// Deserializes a shallow reference value while preserving numeric token
/// semantics.
///
/// Numeric tokens are parsed through the independent strict number reference;
/// nonnumeric values are delegated to the supplied visitor through Serde.
/// Containers are traversed by that visitor, which is responsible for retaining
/// source order and rejecting decoded duplicate keys where required. This
/// helper is intended for finite, shallow fuzz inputs and does not replace a
/// general purpose JSON parser.
pub(crate) fn deserialize_reference_value<'de, D, V, T>(deserializer: D, visitor: V) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    V: for<'raw> Visitor<'raw, Value = T>,
{
    let raw = Box::<serde_json::value::RawValue>::deserialize(deserializer)?;
    let text = raw.get().trim();
    if text
        .as_bytes()
        .first()
        .is_some_and(|byte| matches!(byte, b'-' | b'0'..=b'9'))
    {
        let number = crate::json_number_contract::parse_strict_reference_number(text)
            .map_err(|error| D::Error::custom(format!("strict reference number rejected: {error:?}")))?;
        if text == "-0" || text.bytes().any(|byte| matches!(byte, b'.' | b'e' | b'E')) {
            return visitor.visit_f64::<D::Error>(number.as_f64().expect("reference checked finite float"));
        }
        if text.starts_with('-') {
            return visitor.visit_i64::<D::Error>(number.as_i64().expect("reference checked signed integer"));
        }
        return visitor.visit_u64::<D::Error>(number.as_u64().expect("reference checked unsigned integer"));
    }
    let mut parser = serde_json::Deserializer::from_str(text);
    let result = parser.deserialize_any(visitor).map_err(D::Error::custom)?;
    parser.end().map_err(D::Error::custom)?;
    Ok(result)
}
