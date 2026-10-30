// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Reference classification for the public JSON number contract.

use qubit_json::encode::JsonIntegerSignedness;
use qubit_json::encode::JsonSerializationErrorKind;
use serde_json::Number;
use serde_json::Value;
use serde_json::value::RawValue;

/// Parses one complete JSON number and applies the product's numeric range.
///
/// This independent test reference validates the whole token with `RawValue`
/// before classifying it. It does not use `Value`'s feature-dependent storage
/// range. The caller must provide a finite-size input; parsing may allocate a
/// token copy. Invalid syntax maps to `InvalidNumberRepresentation`, integer
/// overflow preserves signedness, and non-finite floats map to
/// `NonFiniteFloat`.
pub fn parse_strict_reference_number(text: &str) -> Result<Number, JsonSerializationErrorKind> {
    if text.trim() != text
        || text
            .as_bytes()
            .first()
            .is_none_or(|byte| !matches!(byte, b'-' | b'0'..=b'9'))
        || serde_json::from_str::<Box<RawValue>>(text).is_err()
    {
        return Err(JsonSerializationErrorKind::InvalidNumberRepresentation);
    }
    if text == "-0" {
        return Number::from_f64(-0.0).ok_or(JsonSerializationErrorKind::NonFiniteFloat);
    }
    if text.bytes().any(|byte| matches!(byte, b'.' | b'e' | b'E')) {
        let number = text
            .parse::<f64>()
            .map_err(|_| JsonSerializationErrorKind::NonFiniteFloat)?;
        return Number::from_f64(number).ok_or(JsonSerializationErrorKind::NonFiniteFloat);
    }
    if text.starts_with('-') {
        text.parse::<i64>()
            .map(Number::from)
            .map_err(|_| JsonSerializationErrorKind::IntegerOutOfRange {
                signedness: JsonIntegerSignedness::Signed,
            })
    } else {
        text.parse::<u64>()
            .map(Number::from)
            .map_err(|_| JsonSerializationErrorKind::IntegerOutOfRange {
                signedness: JsonIntegerSignedness::Unsigned,
            })
    }
}

/// Normalizes a parsed reference value to the product's numeric contract.
///
/// The input must have finite depth and originate from a syntactically valid
/// JSON document. Arrays, object keys, and nonnumeric values retain their
/// structure; numbers are reparsed through the independent strict reference.
/// The result allocates a fresh tree and returns the exact numeric rejection
/// when a number is outside the product contract.
pub fn normalize_reference_value(value: &Value) -> Result<Value, JsonSerializationErrorKind> {
    match value {
        Value::Number(number) => parse_strict_reference_number(&number.to_string()).map(Value::Number),
        Value::Array(values) => values
            .iter()
            .map(normalize_reference_value)
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Array),
        Value::Object(values) => values
            .iter()
            .map(|(key, value)| normalize_reference_value(value).map(|value| (key.clone(), value)))
            .collect::<Result<serde_json::Map<String, Value>, _>>()
            .map(Value::Object),
        _ => Ok(value.clone()),
    }
}

/// Reports whether every number token fits the `qubit-json` number contract.
///
/// The caller must first establish that `input` is one complete, syntactically
/// valid JSON document. This classifier deliberately relies on that fact and
/// checks only numeric representation; it is not a second JSON parser.
///
/// # Parameters
///
/// * `input` - Syntactically valid JSON bytes whose number tokens are checked.
///
/// # Returns
///
/// `true` when every negative integer fits `i64`, every non-negative integer
/// fits `u64`, and every fractional or exponential token is a finite `f64`.
#[must_use]
pub fn numbers_fit_contract(input: &[u8]) -> bool {
    let mut offset = 0;
    while offset < input.len() {
        match input[offset] {
            b'"' => skip_string(input, &mut offset),
            b'-' | b'0'..=b'9' => {
                let start = offset;
                while offset < input.len() && !is_number_delimiter(input[offset]) {
                    offset += 1;
                }
                if !number_fits_contract(&input[start..offset]) {
                    return false;
                }
            }
            _ => offset += 1,
        }
    }
    true
}

/// Advances `offset` past one valid JSON string, including escaped bytes.
fn skip_string(input: &[u8], offset: &mut usize) {
    *offset += 1;
    while *offset < input.len() {
        match input[*offset] {
            b'\\' => *offset += 2,
            b'"' => {
                *offset += 1;
                return;
            }
            _ => *offset += 1,
        }
    }
}

/// Reports whether a byte terminates a number in syntactically valid JSON.
#[must_use]
const fn is_number_delimiter(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\r' | b'\n' | b',' | b']' | b'}')
}

/// Classifies one syntactically valid JSON number token by its wire shape.
#[must_use]
fn number_fits_contract(token: &[u8]) -> bool {
    let Ok(number) = std::str::from_utf8(token) else {
        return false;
    };
    if token.contains(&b'.') || token.contains(&b'e') || token.contains(&b'E') {
        number.parse::<f64>().is_ok_and(f64::is_finite)
    } else if token.starts_with(b"-") {
        number.parse::<i64>().is_ok()
    } else {
        number.parse::<u64>().is_ok()
    }
}
