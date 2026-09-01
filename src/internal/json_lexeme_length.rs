// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Measures JSON scalar lexemes without allocating temporary strings.

use serde_json::ser::CompactFormatter;
use serde_json::ser::Formatter;

use super::JsonLexemeLengthWriter;

/// Calculates scalar lexeme lengths without constructing JSON values.
pub(crate) enum JsonLexemeLength {}

impl JsonLexemeLength {
    /// Returns the decimal byte length of one signed JSON integer.
    #[inline(always)]
    pub(crate) const fn signed_integer(value: i128) -> usize {
        let sign = if value < 0 { 1 } else { 0 };
        sign + Self::unsigned_integer(value.unsigned_abs())
    }

    /// Returns the decimal byte length of one unsigned JSON integer.
    #[inline(always)]
    pub(crate) const fn unsigned_integer(value: u128) -> usize {
        if value < 10 {
            1
        } else {
            value.ilog10() as usize + 1
        }
    }

    /// Returns the byte length emitted by serde_json for one finite `f32`.
    #[inline]
    pub(crate) fn finite_f32(value: f32) -> usize {
        let mut writer = JsonLexemeLengthWriter::new();
        CompactFormatter
            .write_f32(&mut writer, value)
            .expect("the JSON lexeme length writer must accept a finite f32");
        writer.len()
    }

    /// Returns the byte length emitted by serde_json for one finite `f64`.
    #[inline]
    pub(crate) fn finite_f64(value: f64) -> usize {
        let mut writer = JsonLexemeLengthWriter::new();
        CompactFormatter
            .write_f64(&mut writer, value)
            .expect("the JSON lexeme length writer must accept a finite f64");
        writer.len()
    }

    /// Returns the decimal byte length of one JSON byte-array element.
    #[inline(always)]
    pub(crate) const fn byte(value: u8) -> usize {
        if value < 10 {
            1
        } else if value < 100 {
            2
        } else {
            3
        }
    }
}
