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
///
/// # Examples
///
/// ```
/// use qubit_budget::json::JsonDecodeLimits;
/// use qubit_json::decode::NormalizingJsonDecodePolicy;
/// use qubit_json::decode::NormalizingJsonDecoder;
/// use serde_json::Value;
///
/// let input = "  {\"ok\":true}  ";
/// let mut decoder = NormalizingJsonDecoder::with_limits(
///     NormalizingJsonDecodePolicy::builder().build(),
///     JsonDecodeLimits::new(),
/// );
/// let document = decoder.prepare_str(input)?;
/// assert_eq!(document.as_str(), r#"{"ok":true}"#);
/// assert_eq!(document.raw_input_bytes(), input.len());
/// assert_eq!(document.normalized_input_bytes(), document.as_str().len());
/// let value = decoder.decode_document::<Value>(&document)?;
/// assert_eq!(value["ok"], true);
/// # Ok::<(), qubit_json::decode::JsonDecodeError>(())
/// ```
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
    ///
    /// The returned slice borrows the document. It is the exact text consumed
    /// by later document-based decoding and does not allocate.
    ///
    /// # Returns
    ///
    /// The normalized JSON text.
    #[inline(always)]
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.text.as_ref()
    }

    /// Returns the original input length in bytes.
    ///
    /// This value includes whitespace, a UTF-8 byte-order mark, and any other
    /// input bytes removed or rewritten during normalization.
    ///
    /// # Returns
    ///
    /// The byte length charged for the original input.
    #[inline(always)]
    #[must_use]
    pub const fn raw_input_bytes(&self) -> usize {
        self.raw_input_bytes
    }

    /// Returns the normalized text length in bytes.
    ///
    /// This is the byte length of [`Self::as_str`], after all enabled
    /// normalization has completed.
    ///
    /// # Returns
    ///
    /// The byte length charged for normalized text.
    #[inline(always)]
    #[must_use]
    pub const fn normalized_input_bytes(&self) -> usize {
        self.normalized_input_bytes
    }
}
