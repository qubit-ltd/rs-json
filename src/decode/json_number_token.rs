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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JsonNumberToken<'call> {
    raw: &'call str,
    offset: usize,
}

impl<'call> JsonNumberToken<'call> {
    pub(crate) const fn from_validated(raw: &'call str, offset: usize) -> Self {
        Self { raw, offset }
    }

    /// Returns the original number spelling.
    pub const fn raw(&self) -> &'call str {
        self.raw
    }

    /// Returns the byte offset of the token in its source document.
    pub const fn offset(&self) -> usize {
        self.offset
    }

    /// Converts the token to an integer in the IEEE-754 safe-integer range
    /// without converting through floating point.
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
