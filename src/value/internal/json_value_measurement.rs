// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Maps materialized JSON nodes to their resource measurements.

use qubit_budget::json::JsonMeasurement;
use serde_json::Value;

use super::json_number_lexeme_length;

/// Builds the resource measurement for one JSON node at a root-inclusive depth.
#[must_use]
pub(crate) fn json_value_measurement(value: &Value, depth: usize) -> JsonMeasurement {
    match value {
        Value::Null => JsonMeasurement::Null { depth },
        Value::Bool(_) => JsonMeasurement::Boolean { depth },
        Value::Number(number) => JsonMeasurement::Number {
            depth,
            bytes: json_number_lexeme_length(number),
        },
        Value::String(text) => JsonMeasurement::String {
            depth,
            bytes: text.len(),
        },
        Value::Array(values) => JsonMeasurement::Array {
            depth,
            items: values.len(),
        },
        Value::Object(entries) => JsonMeasurement::Object {
            depth,
            entries: entries.len(),
        },
    }
}
