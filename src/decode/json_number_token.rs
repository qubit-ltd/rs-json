// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Borrowed source JSON number tokens.

use super::JsonNumberTokenError;

/// A syntactically validated number token borrowed from the input document.
///
/// The token is only valid during the synchronous validated-decoding callback
/// that supplied it; its source spelling cannot outlive that callback. Offsets
/// are byte offsets from the beginning of the source document. The exact
/// safe-integer range is the closed interval
/// `[-9007199254740991, 9007199254740991]`. Decimal or exponent spellings are
/// interpreted exactly, without an intermediate floating-point conversion.
///
/// # Examples
///
/// ```
/// use qubit_json::decode::{JsonDecoder, JsonNumberPolicy, JsonValidatedDecodeOptions};
///
/// let options = JsonValidatedDecodeOptions::builder().build();
/// let owner = JsonDecoder::unlimited().decode_unique_validated_str(
///     "1.00e2", &options, |path, token| {
///         assert!(path.is_empty());
///         assert_eq!(token.raw(), "1.00e2");
///         assert_eq!(token.offset(), 0);
///         assert_eq!(token.exact_safe_integer(), Ok(100));
///         Ok::<_, std::convert::Infallible>(JsonNumberPolicy::ExactSafeInteger)
///     })?;
/// assert_eq!(owner.as_value().as_i64(), Some(100));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JsonNumberToken<'call> {
    /// Validated number spelling borrowed for the callback duration.
    raw: &'call str,
    /// Zero-based source byte offset.
    offset: usize,
}

impl<'call> JsonNumberToken<'call> {
    /// Constructs a token view for a number whose JSON syntax the scanner has
    /// validated.
    ///
    /// `raw` must be the exact source spelling of that validated token, and
    /// `offset` its zero-based byte offset in the same input. The borrow must
    /// remain limited to the synchronous policy callback.
    pub(crate) const fn from_validated(raw: &'call str, offset: usize) -> Self {
        Self { raw, offset }
    }

    /// Returns the original number spelling.
    ///
    /// # Returns
    ///
    /// The validated source spelling, borrowed for the callback duration.
    pub const fn raw(&self) -> &'call str {
        self.raw
    }

    /// Returns the zero-based byte offset of the token in its source document.
    pub const fn offset(&self) -> usize {
        self.offset
    }

    /// Converts an exact integral token in the IEEE-754 safe-integer range
    /// without converting through floating point.
    ///
    /// # Returns
    ///
    /// The represented integer when its exact mathematical value is integral
    /// and lies in `[-9007199254740991, 9007199254740991]`.
    ///
    /// # Errors
    ///
    /// Returns [`JsonNumberTokenError::Fractional`] when the exact value has a
    /// fractional part, and [`JsonNumberTokenError::OutOfSafeIntegerRange`]
    /// when the exact integral value lies outside the safe-integer interval.
    pub fn exact_safe_integer(&self) -> Result<i64, JsonNumberTokenError> {
        let bytes = self.raw.as_bytes();
        let negative = bytes.first() == Some(&b'-');
        let start = usize::from(negative);
        let exponent_marker = bytes[start..]
            .iter()
            .position(|byte| matches!(byte, b'e' | b'E'))
            .map(|position| start + position);
        let mantissa_end = exponent_marker.unwrap_or(bytes.len());
        let exponent = exponent_marker.map_or(0_i64, |position| {
            parse_saturated_exponent(&bytes[position + 1..], bytes.len().saturating_add(32) as i64)
        });
        let mut fractional_digits = 0_i64;
        let mut digit_count = 0_usize;
        let mut first_nonzero = None;
        let mut last_nonzero = 0_usize;
        let mut after_decimal = false;
        for byte in &bytes[start..mantissa_end] {
            match byte {
                b'.' => after_decimal = true,
                b'0'..=b'9' => {
                    if after_decimal {
                        fractional_digits = fractional_digits.saturating_add(1);
                    }
                    if *byte != b'0' {
                        first_nonzero.get_or_insert(digit_count);
                        last_nonzero = digit_count;
                    }
                    digit_count += 1;
                }
                _ => unreachable!("token is lexically validated"),
            }
        }
        let Some(first_nonzero) = first_nonzero else {
            return Ok(0);
        };
        let shift = exponent.saturating_sub(fractional_digits);
        let trailing_zeroes = digit_count - last_nonzero - 1;
        let (last_kept, zeroes_to_append) = if shift < 0 {
            let remove = shift.unsigned_abs() as usize;
            if remove > trailing_zeroes {
                return Err(JsonNumberTokenError::Fractional);
            }
            (digit_count - 1 - remove, 0_usize)
        } else {
            (digit_count - 1, shift as usize)
        };
        let resulting_len = last_kept - first_nonzero + 1 + zeroes_to_append;
        if resulting_len > 16 {
            return Err(JsonNumberTokenError::OutOfSafeIntegerRange);
        }
        let mut magnitude = 0_u64;
        let mut ordinal = 0_usize;
        for byte in &bytes[start..mantissa_end] {
            if byte.is_ascii_digit() {
                if (first_nonzero..=last_kept).contains(&ordinal) {
                    magnitude = magnitude * 10 + u64::from(*byte - b'0');
                }
                ordinal += 1;
            }
        }
        for _ in 0..zeroes_to_append {
            magnitude *= 10;
        }
        const MAX_SAFE: u64 = 9_007_199_254_740_991;
        if magnitude > MAX_SAFE {
            return Err(JsonNumberTokenError::OutOfSafeIntegerRange);
        }
        let value = magnitude as i64;
        Ok(if negative { -value } else { value })
    }
}

/// Parses validated exponent digits while bounding magnitude to avoid overflow.
fn parse_saturated_exponent(bytes: &[u8], bound: i64) -> i64 {
    let negative = bytes.first() == Some(&b'-');
    let digits = if matches!(bytes.first(), Some(b'-' | b'+')) {
        &bytes[1..]
    } else {
        bytes
    };
    let mut value = 0_i64;
    for byte in digits {
        value = value
            .saturating_mul(10)
            .saturating_add(i64::from(byte - b'0'))
            .min(bound);
    }
    if negative { -value } else { value }
}
