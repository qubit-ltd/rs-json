// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines normalized JSON text retained for borrowing deserialization.

use std::borrow::Cow;

/// A normalized JSON document whose text outlives borrowed decode results.
///
/// Preparing a document charges raw and normalized input budgets. Decoding it
/// later charges only decoded-value budgets, and the same document may be
/// decoded repeatedly. Documents are detached from the decoder that prepared
/// them; value charges belong to the decoder performing each decode.
///
/// Borrowing follows Serde's JSON representation rules: strings without JSON
/// escapes can borrow from this document, while strings containing escapes
/// require an owned target because deserialization must materialize their
/// unescaped contents.
#[derive(Debug, Clone)]
pub struct NormalizedJsonDocument<'input> {
    /// Normalized text, borrowed when rewriting did not require allocation.
    text: Cow<'input, str>,
    /// Original input length in bytes.
    raw_input_bytes: usize,
    /// Normalized text length in bytes.
    normalized_input_bytes: usize,
}

impl<'input> NormalizedJsonDocument<'input> {
    /// Creates a document from normalized text and its original byte length.
    #[inline]
    #[must_use]
    pub(in crate::decode) fn new(text: Cow<'input, str>, raw_input_bytes: usize) -> Self {
        let normalized_input_bytes = text.len();
        Self {
            text,
            raw_input_bytes,
            normalized_input_bytes,
        }
    }

    /// Returns the normalized JSON text retained by this document.
    #[inline(always)]
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.text.as_ref()
    }

    /// Returns the original input length in bytes.
    #[inline(always)]
    #[must_use]
    pub const fn raw_input_bytes(&self) -> usize {
        self.raw_input_bytes
    }

    /// Returns the normalized text length in bytes.
    #[inline(always)]
    #[must_use]
    pub const fn normalized_input_bytes(&self) -> usize {
        self.normalized_input_bytes
    }
}
