// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Reference classification for the public JSON number contract.

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
