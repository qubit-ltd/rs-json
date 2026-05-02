/*******************************************************************************
 *
 *    Copyright (c) 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
//! Defines the option type used to configure the lenient JSON decoder.
//!

/// Configuration switches for [`crate::LenientJsonDecoder`].
///
/// Each flag controls one normalization rule applied before parsing JSON.
/// Defaults are intentionally conservative and cover the most common
/// non-fully-trusted text inputs without attempting aggressive repair.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JsonDecodeOptions {
    /// Controls whether leading and trailing whitespace is removed before any
    /// other normalization step is applied.
    pub trim_whitespace: bool,
    /// Controls whether a leading UTF-8 byte order mark (`U+FEFF`) is removed
    /// before parsing.
    pub strip_utf8_bom: bool,
    /// Controls whether one outer Markdown code fence is removed.
    ///
    /// Typical examples include `````json ... ````` and bare fenced blocks
    /// starting with ````` followed by a newline.
    pub strip_markdown_code_fence: bool,
    /// Controls whether Markdown fence stripping requires a matching closing
    /// fence.
    ///
    /// When enabled, an opening fence without a valid closing fence keeps the
    /// input unchanged.
    pub strip_markdown_code_fence_requires_closing: bool,
    /// Controls whether Markdown fence stripping only accepts JSON-like info
    /// strings whose first token is empty, `json`, or `jsonc`.
    ///
    /// When enabled, fenced blocks labeled with other languages are not
    /// stripped.
    pub strip_markdown_code_fence_json_only: bool,
    /// Controls whether raw ASCII control characters inside JSON string
    /// literals are converted into valid JSON escape sequences.
    pub escape_control_chars_in_strings: bool,
    /// Caps the accepted raw input size in bytes before normalization.
    ///
    /// When set to `Some(limit)`, any input whose byte length is greater than
    /// `limit` is rejected before further processing. When set to `None`,
    /// no size limit is enforced.
    pub max_input_bytes: Option<usize>,
}

impl JsonDecodeOptions {
    /// Creates the default lenient option set.
    ///
    /// This preset enables the small, predictable normalization rules intended
    /// for non-fully-trusted text inputs while keeping aggressive JSON repair out
    /// of scope.
    #[must_use]
    pub const fn lenient() -> Self {
        Self {
            trim_whitespace: true,
            strip_utf8_bom: true,
            strip_markdown_code_fence: true,
            strip_markdown_code_fence_requires_closing: false,
            strip_markdown_code_fence_json_only: false,
            escape_control_chars_in_strings: true,
            max_input_bytes: None,
        }
    }

    /// Creates an option set that disables all normalization rules.
    ///
    /// This preset still allows `serde_json` to accept JSON syntax that is valid
    /// on its own, but the decoder will not trim, strip BOMs, remove Markdown
    /// fences, or escape raw control characters before parsing.
    #[must_use]
    pub const fn strict() -> Self {
        Self {
            trim_whitespace: false,
            strip_utf8_bom: false,
            strip_markdown_code_fence: false,
            strip_markdown_code_fence_requires_closing: false,
            strip_markdown_code_fence_json_only: false,
            escape_control_chars_in_strings: false,
            max_input_bytes: None,
        }
    }

    /// Creates lenient options that only strip JSON-like Markdown code fences.
    ///
    /// The resulting preset keeps the other default normalization rules, but
    /// restricts Markdown fence stripping to empty, `json`, or `jsonc` as the
    /// first info-string token.
    #[must_use]
    pub const fn json_code_fences_only() -> Self {
        let mut options = Self::lenient();
        options.strip_markdown_code_fence_json_only = true;
        options
    }

    /// Returns a copy of these options with a raw input byte-size limit.
    ///
    /// Inputs whose byte length is greater than `max_input_bytes` are rejected
    /// before normalization.
    #[must_use]
    pub const fn with_max_input_bytes(mut self, max_input_bytes: usize) -> Self {
        self.max_input_bytes = Some(max_input_bytes);
        self
    }
}

impl Default for JsonDecodeOptions {
    fn default() -> Self {
        Self::lenient()
    }
}
