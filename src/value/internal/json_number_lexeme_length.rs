// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Measures representable serde_json number lexemes without allocation.

use serde_json::Number;

use crate::internal::JsonLexemeLength;

/// Returns the compact JSON byte length emitted for one representable number.
///
/// This helper follows the crate's standard `i64`, `u64`, and finite `f64`
/// value model and performs no allocation.
pub(crate) fn json_number_lexeme_length(number: &Number) -> usize {
    if number.is_i64() {
        let value = number.as_i64().expect("an i64-classified JSON number must expose i64");
        let sign = usize::from(value.is_negative());
        return sign + JsonLexemeLength::unsigned_integer(value.unsigned_abs().into());
    }
    if number.is_u64() {
        let value = number.as_u64().expect("a u64-classified JSON number must expose u64");
        return JsonLexemeLength::unsigned_integer(value.into());
    }
    let value = number.as_f64().expect("a finite JSON number must expose f64");
    JsonLexemeLength::finite_f64(value)
}
