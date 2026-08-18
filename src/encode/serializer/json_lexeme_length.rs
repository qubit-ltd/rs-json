// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Measures JSON scalar lexemes without allocating temporary strings.

/// Calculates scalar lexeme lengths without constructing JSON values.
pub(super) enum JsonLexemeLength {}

impl JsonLexemeLength {
    /// Returns the decimal byte length of one signed JSON integer.
    #[inline(always)]
    pub(super) const fn signed_integer(value: i128) -> usize {
        let sign = if value < 0 { 1 } else { 0 };
        sign + Self::unsigned_integer(value.unsigned_abs())
    }

    /// Returns the decimal byte length of one unsigned JSON integer.
    #[inline(always)]
    pub(super) const fn unsigned_integer(value: u128) -> usize {
        if value < 10 { 1 } else { value.ilog10() as usize + 1 }
    }

    /// Returns the byte length of one finite `f32` JSON number.
    #[inline(always)]
    pub(super) fn finite_f32(value: f32) -> usize {
        let mut buffer = zmij::Buffer::new();
        buffer.format_finite(value).len()
    }

    /// Returns the byte length of one finite `f64` JSON number.
    #[inline(always)]
    pub(super) fn finite_f64(value: f64) -> usize {
        let mut buffer = zmij::Buffer::new();
        buffer.format_finite(value).len()
    }

    /// Returns the decimal byte length of one JSON byte-array element.
    #[inline(always)]
    pub(super) const fn byte(value: u8) -> usize {
        if value < 10 {
            1
        } else if value < 100 {
            2
        } else {
            3
        }
    }
}
