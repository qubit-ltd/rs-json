// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines the option type used to configure the lenient JSON decoder.

use crate::ErrorPrivacyPolicy;
use crate::MarkdownFenceClosing;
use crate::MarkdownFencePolicy;

/// Configuration switches for [`crate::LenientJsonDecoder`].
///
/// Its fields control text normalization, input limits, and error
/// diagnostics. Defaults are intentionally conservative and cover the most
/// common non-fully-trusted text inputs without attempting aggressive repair.
///
/// # Examples
///
/// ```compile_fail
/// use qubit_json::{JsonDecodeOptions, LenientJsonDecoder};
///
/// let options = JsonDecodeOptions::strict();
/// let _decoder = LenientJsonDecoder::new(options);
/// let _moved_options = options;
/// ```
#[must_use = "JSON decoding options have no effect until used to construct a decoder"]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonDecodeOptions {
    /// Controls whether leading and trailing whitespace is removed before any
    /// other normalization step is applied.
    trim_whitespace: bool,
    /// Controls whether a leading UTF-8 byte order mark (`U+FEFF`) is removed
    /// before parsing.
    strip_utf8_bom: bool,
    /// Controls whether and how one outer Markdown code fence is removed.
    markdown_fence_policy: MarkdownFencePolicy,
    /// Controls whether raw ASCII control characters inside JSON string
    /// literals are converted into valid JSON escape sequences.
    escape_control_chars_in_strings: bool,
    /// Caps the accepted raw input size in bytes before normalization.
    ///
    /// When set to `Some(limit)`, any input whose byte length is greater than
    /// `limit` is rejected before further processing. When set to `None`, no
    /// size limit is enforced. This does not bound normalized allocation size:
    /// escaping a raw control byte can expand it to six ASCII bytes.
    max_input_bytes: Option<usize>,
    /// Caps the normalized JSON byte size before the decoder allocates
    /// repaired text for raw control characters.
    ///
    /// When set to `Some(limit)`, the final normalized text must not exceed
    /// `limit` bytes. When set to `None`, no normalized-size limit is
    /// enforced.
    max_normalized_bytes: Option<usize>,
    /// Controls whether decoding errors retain input-derived serde details.
    error_privacy_policy: ErrorPrivacyPolicy,
}

impl JsonDecodeOptions {
    /// Creates the default lenient option set.
    ///
    /// # Returns
    ///
    /// An option set that enables every supported normalization rule, applies
    /// no input-size limit, and redacts input-derived error details.
    #[inline]
    pub const fn lenient() -> Self {
        Self {
            trim_whitespace: true,
            strip_utf8_bom: true,
            markdown_fence_policy: MarkdownFencePolicy::JsonOnly {
                closing: MarkdownFenceClosing::Optional,
            },
            escape_control_chars_in_strings: true,
            max_input_bytes: None,
            max_normalized_bytes: None,
            error_privacy_policy: ErrorPrivacyPolicy::Redacted,
        }
    }

    /// Creates an option set that disables all text-rewriting rules.
    ///
    /// The decoder still applies empty-input classification, optional raw and
    /// normalized input-size limits, the configured privacy policy, and stable
    /// error mapping before or around parsing and deserialization.
    ///
    /// # Returns
    ///
    /// An option set that leaves input text unchanged, applies no raw or
    /// normalized size limit,
    /// delegates parsing and deserialization to `serde_json`, and redacts
    /// input-derived error details.
    #[inline]
    pub const fn strict() -> Self {
        Self {
            trim_whitespace: false,
            strip_utf8_bom: false,
            markdown_fence_policy: MarkdownFencePolicy::Disabled,
            escape_control_chars_in_strings: false,
            max_input_bytes: None,
            max_normalized_bytes: None,
            error_privacy_policy: ErrorPrivacyPolicy::Redacted,
        }
    }

    /// Returns whether leading and trailing whitespace is removed.
    ///
    /// # Returns
    ///
    /// `true` when surrounding whitespace is removed; otherwise, `false`.
    #[inline(always)]
    #[must_use]
    pub const fn trim_whitespace(&self) -> bool {
        self.trim_whitespace
    }

    /// Returns these options with whitespace trimming enabled or disabled.
    ///
    /// # Parameters
    ///
    /// * `enabled` - Whether to remove surrounding whitespace.
    ///
    /// # Returns
    ///
    /// The updated option set.
    #[inline(always)]
    pub const fn with_trim_whitespace(mut self, enabled: bool) -> Self {
        self.trim_whitespace = enabled;
        self
    }

    /// Returns whether a leading UTF-8 byte order mark is removed.
    ///
    /// # Returns
    ///
    /// `true` when a leading UTF-8 byte order mark is removed; otherwise,
    /// `false`.
    #[inline(always)]
    #[must_use]
    pub const fn strip_utf8_bom(&self) -> bool {
        self.strip_utf8_bom
    }

    /// Returns these options with UTF-8 byte order mark stripping configured.
    ///
    /// # Parameters
    ///
    /// * `enabled` - Whether to remove a leading UTF-8 byte order mark.
    ///
    /// # Returns
    ///
    /// The updated option set.
    #[inline(always)]
    pub const fn with_strip_utf8_bom(mut self, enabled: bool) -> Self {
        self.strip_utf8_bom = enabled;
        self
    }

    /// Returns the policy used to remove one outer Markdown code fence.
    ///
    /// # Returns
    ///
    /// The configured Markdown fence policy.
    ///
    /// # Examples
    ///
    /// ```compile_fail
    /// #![deny(unused_must_use)]
    /// use qubit_json::JsonDecodeOptions;
    ///
    /// let options = JsonDecodeOptions::strict();
    /// options.markdown_fence_policy();
    /// ```
    #[inline(always)]
    #[must_use = "the configured Markdown fence policy should be inspected"]
    pub const fn markdown_fence_policy(&self) -> &MarkdownFencePolicy {
        &self.markdown_fence_policy
    }

    /// Returns these options with a Markdown fence policy.
    ///
    /// # Parameters
    ///
    /// * `markdown_fence_policy` - Policy used to recognize and remove one
    ///   outer Markdown code fence.
    ///
    /// # Returns
    ///
    /// The updated option set.
    #[inline(always)]
    pub const fn with_markdown_fence_policy(
        mut self,
        markdown_fence_policy: MarkdownFencePolicy,
    ) -> Self {
        self.markdown_fence_policy = markdown_fence_policy;
        self
    }

    /// Returns whether raw control characters in JSON strings are escaped.
    ///
    /// # Returns
    ///
    /// `true` when raw ASCII control characters inside JSON strings are
    /// escaped; otherwise, `false`.
    #[inline(always)]
    #[must_use]
    pub const fn escape_control_chars_in_strings(&self) -> bool {
        self.escape_control_chars_in_strings
    }

    /// Returns these options with JSON-string control character escaping
    /// configured.
    ///
    /// # Parameters
    ///
    /// * `enabled` - Whether to escape raw ASCII control characters inside JSON
    ///   strings.
    ///
    /// # Returns
    ///
    /// The updated option set.
    #[inline(always)]
    pub const fn with_escape_control_chars_in_strings(
        mut self,
        enabled: bool,
    ) -> Self {
        self.escape_control_chars_in_strings = enabled;
        self
    }

    /// Returns the raw input byte-size limit.
    ///
    /// # Returns
    ///
    /// `Some(limit)` when accepted raw input is capped at `limit` bytes, or
    /// `None` when the decoder enforces no input-size limit.
    #[inline(always)]
    pub const fn max_input_bytes(&self) -> Option<usize> {
        self.max_input_bytes
    }

    /// Returns these options with a raw input byte-size limit.
    ///
    /// # Parameters
    ///
    /// * `max_input_bytes` - `Some(limit)` to cap the raw input at `limit`
    ///   bytes, or `None` to remove the limit.
    ///
    /// # Returns
    ///
    /// The updated option set.
    #[inline(always)]
    pub const fn with_max_input_bytes(
        mut self,
        max_input_bytes: Option<usize>,
    ) -> Self {
        self.max_input_bytes = max_input_bytes;
        self
    }

    /// Returns the normalized JSON byte-size limit.
    ///
    /// # Returns
    ///
    /// `Some(limit)` when normalized JSON is capped at `limit` bytes, or
    /// `None` when the decoder enforces no normalized-size limit.
    #[inline(always)]
    pub const fn max_normalized_bytes(&self) -> Option<usize> {
        self.max_normalized_bytes
    }

    /// Returns these options with a normalized JSON byte-size limit.
    ///
    /// The decoder calculates the normalized size before allocating repaired
    /// text for raw control characters, so this limit also bounds the
    /// allocation caused by supported control-character escaping.
    ///
    /// # Parameters
    ///
    /// * `max_normalized_bytes` - `Some(limit)` to cap normalized JSON at
    ///   `limit` bytes, or `None` to remove the limit.
    ///
    /// # Returns
    ///
    /// The updated option set.
    #[inline(always)]
    pub const fn with_max_normalized_bytes(
        mut self,
        max_normalized_bytes: Option<usize>,
    ) -> Self {
        self.max_normalized_bytes = max_normalized_bytes;
        self
    }

    /// Returns the privacy policy applied to decoding error diagnostics.
    ///
    /// # Returns
    ///
    /// The configured error privacy policy.
    #[inline(always)]
    pub const fn error_privacy_policy(&self) -> ErrorPrivacyPolicy {
        self.error_privacy_policy
    }

    /// Returns these options with the requested error privacy policy.
    ///
    /// The policy determines whether serde diagnostics derived from input
    /// values are retained in returned errors.
    ///
    /// # Parameters
    ///
    /// * `error_privacy_policy` - Policy applied when constructing decoding
    ///   errors.
    ///
    /// # Returns
    ///
    /// The updated option set.
    #[inline(always)]
    pub const fn with_error_privacy_policy(
        mut self,
        error_privacy_policy: ErrorPrivacyPolicy,
    ) -> Self {
        self.error_privacy_policy = error_privacy_policy;
        self
    }
}

impl Default for JsonDecodeOptions {
    /// Creates the default lenient option set.
    ///
    /// # Returns
    ///
    /// The same option set as [`Self::lenient`].
    #[inline(always)]
    fn default() -> Self {
        Self::lenient()
    }
}
