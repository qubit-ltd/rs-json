// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Measures representable serde_json number lexemes without allocation.

use std::fmt::Write as _;

use serde_json::Number;

use super::JsonNumberLengthWriter;

/// Returns the compact JSON byte length emitted for one representable number.
///
/// This helper counts the exact representation used by `Number` and performs no
/// allocation.
///
/// # Parameters
///
/// * `number` - Representable JSON number whose display form is counted.
///
/// # Returns
///
/// The number of UTF-8 bytes in the compact JSON number representation.
#[must_use]
pub(crate) fn json_number_lexeme_length(number: &Number) -> usize {
    let mut writer = JsonNumberLengthWriter::new();
    let result = write!(&mut writer, "{number}");
    debug_assert!(result.is_ok(), "Number Display only writes to an infallible counter");
    writer.len()
}
