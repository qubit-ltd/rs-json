// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Number handling choices for validated decoding.

/// Selects how a validated source number is represented in the output tree.
///
/// [`ExactSafeInteger`](Self::ExactSafeInteger) accepts only exact mathematical
/// integer values in the inclusive interval
/// `[-9007199254740991, 9007199254740991]`. It checks the decimal value
/// directly, without rounding through `f64`, and projects an accepted value to
/// `i64`. Thus, `1.00e2` is accepted as `100`, while fractional values and
/// integral values outside that interval are rejected as distinct
/// [`JsonNumberTokenError`](super::JsonNumberTokenError) categories.
///
/// After the scanner recognizes a valid number token, it calls the number
/// policy before admitting that number's resource measurements. It then
/// checks the selected decode model's range and applies the selected
/// projection. A policy error returns immediately and can therefore precede
/// a number-resource budget error.
///
/// Earlier input, depth, container, and validated-option checks can reject the
/// document before the callback. A successful callback does not imply that
/// resource admission or validation of the complete document succeeded; its
/// side effects are not rolled back after a later failure. Selecting
/// `ExactSafeInteger` does not bypass the decode model range check, so a
/// spelling such as `1e400` can fail first as
/// [`JsonValidatedDecodeError::Decode`](super::JsonValidatedDecodeError::Decode)
/// with [`JsonSyntaxErrorReason::FloatOutOfRange`](super::JsonSyntaxErrorReason::FloatOutOfRange).
///
/// [`JsonNumberToken::exact_safe_integer`](super::JsonNumberToken::exact_safe_integer)
/// exposes the same exact conversion for use inside a policy callback.
///
/// # Examples
///
/// ```
/// use qubit_json::decode::{
///     JsonDecoder, JsonNumberPolicy, JsonNumberTokenError,
///     JsonSyntaxErrorReason, JsonValidatedDecodeError,
///     JsonValidatedDecodeOptions,
/// };
/// use std::convert::Infallible;
///
/// let options = JsonValidatedDecodeOptions::builder().build();
/// let decode = |text| {
///     JsonDecoder::unlimited().decode_unique_validated_str(
///         text,
///         &options,
///         |_, _| Ok::<_, Infallible>(JsonNumberPolicy::ExactSafeInteger),
///     )
/// };
///
/// let owner = decode("1.00e2").unwrap();
/// assert_eq!(owner.as_value().as_i64(), Some(100));
/// assert!(matches!(
///     decode("0.1"),
///     Err(JsonValidatedDecodeError::ExactSafeInteger {
///         source: JsonNumberTokenError::Fractional,
///         ..
///     })
/// ));
/// assert!(matches!(
///     decode("1e400"),
///     Err(JsonValidatedDecodeError::Decode { error, .. })
///         if error.syntax_error().map(|syntax| syntax.reason())
///             == Some(JsonSyntaxErrorReason::FloatOutOfRange)
/// ));
/// ```
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JsonNumberPolicy {
    /// Preserve the number according to the configured decode model.
    Preserve,
    /// Require an exact integer in JavaScript's safe integer range.
    ///
    /// The range is `[-9007199254740991, 9007199254740991]`. The decimal
    /// mathematical value is checked without an intermediate `f64` rounding
    /// step, then projected to `i64`. Fractional values produce
    /// [`JsonNumberTokenError::Fractional`](super::JsonNumberTokenError::Fractional);
    /// integral values outside the range produce
    /// [`JsonNumberTokenError::OutOfSafeIntegerRange`](super::JsonNumberTokenError::OutOfSafeIntegerRange).
    ExactSafeInteger,
}
