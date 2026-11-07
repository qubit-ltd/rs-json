// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Errors from exact-safe-integer conversion.

use std::error::Error;
use std::fmt::Display;
use std::fmt::Formatter;
use std::fmt::Result;

/// Explains why a validated JSON number cannot be represented as a safe
/// integer.
///
/// `JsonNumberToken::exact_safe_integer` returns this error when the exact
/// decimal value is fractional or outside the IEEE-754 safe-integer range.
///
/// # Examples
///
/// ```
/// use qubit_json::decode::JsonNumberTokenError;
///
/// let error = JsonNumberTokenError::Fractional;
/// assert_eq!(error.to_string(), "number is fractional");
/// ```
#[must_use]
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JsonNumberTokenError {
    /// The token has a non-integral mathematical value.
    Fractional,
    /// The integer is outside the inclusive safe-integer range.
    OutOfSafeIntegerRange,
}

impl Display for JsonNumberTokenError {
    /// Formats the error as a concise description of the failed conversion.
    ///
    /// Fractional values and integers outside the safe range have distinct
    /// messages so callers can preserve the failure category in diagnostics.
    ///
    /// # Parameters
    ///
    /// - `formatter`: Receives the formatted error text.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` when the message is written.
    ///
    /// # Errors
    ///
    /// Returns the formatter's error if writing the message fails.
    fn fmt(&self, formatter: &mut Formatter<'_>) -> Result {
        match self {
            Self::Fractional => formatter.write_str("number is fractional"),
            Self::OutOfSafeIntegerRange => formatter.write_str("integer is outside the safe range"),
        }
    }
}

impl Error for JsonNumberTokenError {}
