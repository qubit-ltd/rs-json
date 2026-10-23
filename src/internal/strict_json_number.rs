// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Strictly admits and constructs JSON numbers using the shared lexical
//! scanner.

use qubit_budget::json::JsonResource;
use qubit_budget::json::JsonValueBudget;
use qubit_budget::json::JsonValueLimits;
use serde_json::Number;

use crate::encode::JsonIntegerSignedness;
use crate::encode::JsonSerializationError;
use crate::encode::JsonSerializationErrorKind;
use crate::lexical::JsonLexicalError;
use crate::lexical::JsonLexicalErrorReason;
use crate::lexical::JsonLexicalScanner;
use crate::lexical::JsonNumberModel;

/// Strict conversions for the ordinary JSON 64-bit integer / finite-f64
/// contract.
pub(crate) enum StrictJsonNumber {}

impl StrictJsonNumber {
    /// Validates exactly one number token before constructing its `Number`
    /// value.
    ///
    /// # Parameters
    ///
    /// * `text` - The candidate JSON number token.
    ///
    /// # Returns
    ///
    /// The corresponding JSON number when the token satisfies the strict
    /// integer and finite-float contract.
    ///
    /// # Errors
    ///
    /// Returns an error when the token is invalid or its value is outside the
    /// supported integer or finite-float range.
    pub(crate) fn parse(text: &str) -> Result<Number, JsonSerializationError> {
        let bytes = text.as_bytes();
        if bytes.first().is_none_or(|byte| !matches!(byte, b'-' | b'0'..=b'9'))
            || bytes.last().is_some_and(u8::is_ascii_whitespace)
        {
            return Err(invalid_number());
        }
        let limits = JsonValueLimits::<JsonResource, usize>::default();
        let mut budget = JsonValueBudget::new(limits);
        let mut transaction = budget.transaction();
        let mut scanner =
            JsonLexicalScanner::new(&mut transaction, false).with_number_model(JsonNumberModel::Integer64);
        match scanner.scan(bytes) {
            Ok(()) => Self::from_admitted_lexeme(text),
            Err(JsonLexicalError::Syntax(failure)) => match failure.reason {
                JsonLexicalErrorReason::IntegerOutOfRange => Err(integer_out_of_range(bytes)),
                JsonLexicalErrorReason::FloatOutOfRange => {
                    Err(number_error(JsonSerializationErrorKind::NonFiniteFloat))
                }
                _ => Err(invalid_number()),
            },
            Err(JsonLexicalError::Budget(_)) => Err(invalid_number()),
        }
    }

    /// Constructs a number from a lexeme already admitted by the lexical
    /// scanner.
    ///
    /// # Parameters
    ///
    /// * `text` - A JSON number lexeme already accepted by the lexical scanner.
    ///
    /// # Returns
    ///
    /// The JSON number represented by the lexeme.
    ///
    /// # Errors
    ///
    /// Returns an error when the lexeme cannot be represented by the supported
    /// integer or finite-float number type.
    pub(crate) fn from_admitted_lexeme(text: &str) -> Result<Number, JsonSerializationError> {
        if text == "-0" {
            return Number::from_f64(-0.0).ok_or_else(invalid_number);
        }
        if text.bytes().any(|byte| matches!(byte, b'.' | b'e' | b'E')) {
            let value = text.parse::<f64>().map_err(|_| invalid_number())?;
            if !value.is_finite() {
                return Err(number_error(JsonSerializationErrorKind::NonFiniteFloat));
            }
            return Number::from_f64(value).ok_or_else(invalid_number);
        }
        if text.starts_with('-') {
            let value = text
                .parse::<i64>()
                .map_err(|_| integer_error(JsonIntegerSignedness::Signed))?;
            Ok(Number::from(value))
        } else {
            let value = text
                .parse::<u64>()
                .map_err(|_| integer_error(JsonIntegerSignedness::Unsigned))?;
            Ok(Number::from(value))
        }
    }

    /// Converts an i128 only when it belongs to the strict signed/unsigned
    /// 64-bit union.
    ///
    /// # Parameters
    ///
    /// * `value` - The signed integer to convert.
    ///
    /// # Returns
    ///
    /// The corresponding JSON integer.
    ///
    /// # Errors
    ///
    /// Returns an error when `value` is outside the signed and unsigned 64-bit
    /// integer ranges.
    pub(crate) fn from_i128(value: i128) -> Result<Number, JsonSerializationError> {
        if let Ok(value) = i64::try_from(value) {
            return Ok(Number::from(value));
        }
        if let Ok(value) = u64::try_from(value) {
            return Ok(Number::from(value));
        }
        Err(integer_error(JsonIntegerSignedness::Signed))
    }

    /// Converts a u128 only when it fits in u64.
    ///
    /// # Parameters
    ///
    /// * `value` - The unsigned integer to convert.
    ///
    /// # Returns
    ///
    /// The corresponding JSON integer.
    ///
    /// # Errors
    ///
    /// Returns an error when `value` exceeds the unsigned 64-bit integer range.
    pub(crate) fn from_u128(value: u128) -> Result<Number, JsonSerializationError> {
        let value = u64::try_from(value).map_err(|_| integer_error(JsonIntegerSignedness::Unsigned))?;
        Ok(Number::from(value))
    }
}

/// Creates the error used for an invalid number representation.
///
/// # Returns
///
/// The invalid-number serialization error.
fn invalid_number() -> JsonSerializationError {
    number_error(JsonSerializationErrorKind::InvalidNumberRepresentation)
}

/// Creates an integer range error using the token's sign to determine
/// signedness.
///
/// # Parameters
///
/// * `bytes` — Number-token bytes whose leading sign selects signedness.
///
/// # Returns
///
/// The integer-out-of-range serialization error for that signedness.
fn integer_out_of_range(bytes: &[u8]) -> JsonSerializationError {
    let signedness = if bytes.first() == Some(&b'-') {
        JsonIntegerSignedness::Signed
    } else {
        JsonIntegerSignedness::Unsigned
    };
    integer_error(signedness)
}

/// Creates an integer range error for the specified integer signedness.
///
/// # Parameters
///
/// * `signedness` — Integer signedness to include in the error.
///
/// # Returns
///
/// The integer-out-of-range serialization error.
fn integer_error(signedness: JsonIntegerSignedness) -> JsonSerializationError {
    number_error(JsonSerializationErrorKind::IntegerOutOfRange { signedness })
}

/// Creates a serialization error with the specified number error kind.
///
/// # Parameters
///
/// * `kind` — Number-related serialization error kind.
///
/// # Returns
///
/// The serialization error wrapping `kind`.
fn number_error(kind: JsonSerializationErrorKind) -> JsonSerializationError {
    JsonSerializationError::new(kind)
}
