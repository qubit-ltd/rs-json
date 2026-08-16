// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines the options used to configure the lenient JSON decoder.
// qubit-style: allow multiple-public-types

use qubit_budget::json::JsonValueLimits;

use super::ErrorPrivacyPolicy;
use super::MarkdownFenceClosing;
use super::MarkdownFencePolicy;

/// Configuration switches for [`crate::lenient::LenientJsonDecoder`].
///
/// Its fields control text normalization, input limits, optional value-resource
/// limits, and error diagnostics. Defaults are intentionally conservative and
/// cover the most common non-fully-trusted text inputs without attempting
/// aggressive repair.
///
/// Convenience entry points such as
/// [`crate::lenient::LenientJsonDecoder::decode`] enforce only the configured
/// raw and normalized input byte limits. They do not perform lexical value
/// admission unless [`Self::value_limits`] is configured. For cumulative
/// multi-value accounting, or when sharing one session across several decode
/// operations, use [`crate::lenient::LenientJsonDecoder::decode_with_session`].
///
/// # Examples
///
/// ```compile_fail
/// use qubit_json::lenient::{LenientJsonDecodeOptions, LenientJsonDecoder};
///
/// let options = LenientJsonDecodeOptions::strict();
/// let _decoder = LenientJsonDecoder::new(options);
/// let _moved_options = options;
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LenientJsonDecodeOptions {
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
    /// Optional JSON value-resource limits enforced by lexical admission.
    ///
    /// When set, convenience decode entry points perform the same lexical
    /// admission as
    /// [`crate::lenient::LenientJsonDecoder::decode_with_session`]
    /// before parsing or deserialization. When unset, only raw and normalized
    /// input byte limits apply on those entry points.
    value_limits: Option<JsonValueLimits>,
    /// Controls whether decoding errors retain input-derived serde details.
    error_privacy_policy: ErrorPrivacyPolicy,
}

impl LenientJsonDecodeOptions {
    /// Creates the default lenient option set.
    ///
    /// # Returns
    ///
    /// An option set that enables every supported normalization rule, applies
    /// no input-size limit, and redacts input-derived error details.
    #[inline]
    #[must_use]
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
            value_limits: None,
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
    #[must_use]
    pub const fn strict() -> Self {
        Self {
            trim_whitespace: false,
            strip_utf8_bom: false,
            markdown_fence_policy: MarkdownFencePolicy::Disabled,
            escape_control_chars_in_strings: false,
            max_input_bytes: None,
            max_normalized_bytes: None,
            value_limits: None,
            error_privacy_policy: ErrorPrivacyPolicy::Redacted,
        }
    }

    /// Creates a builder initialized with the default lenient options.
    #[inline]
    #[must_use]
    pub const fn builder() -> LenientJsonDecodeOptionsBuilder {
        LenientJsonDecodeOptionsBuilder::new()
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
    /// use qubit_json::lenient::LenientJsonDecodeOptions;
    ///
    /// let options = LenientJsonDecodeOptions::strict();
    /// options.markdown_fence_policy();
    /// ```
    #[inline(always)]
    #[must_use]
    pub const fn markdown_fence_policy(&self) -> &MarkdownFencePolicy {
        &self.markdown_fence_policy
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

    /// Returns the raw input byte-size limit.
    ///
    /// # Returns
    ///
    /// `Some(limit)` when accepted raw input is capped at `limit` bytes, or
    /// `None` when the decoder enforces no input-size limit.
    #[inline(always)]
    #[must_use]
    pub const fn max_input_bytes(&self) -> Option<usize> {
        self.max_input_bytes
    }

    /// Returns the normalized JSON byte-size limit.
    ///
    /// # Returns
    ///
    /// `Some(limit)` when normalized JSON is capped at `limit` bytes, or
    /// `None` when the decoder enforces no normalized-size limit.
    #[inline(always)]
    #[must_use]
    pub const fn max_normalized_bytes(&self) -> Option<usize> {
        self.max_normalized_bytes
    }

    /// Returns the configured JSON value-resource limits.
    ///
    /// # Returns
    ///
    /// `Some(limits)` when convenience decode entry points perform lexical
    /// value admission, or `None` when only input byte limits apply.
    #[inline(always)]
    #[must_use]
    pub const fn value_limits(&self) -> Option<JsonValueLimits> {
        self.value_limits
    }

    /// Returns the privacy policy applied to decoding error diagnostics.
    ///
    /// # Returns
    ///
    /// The configured error privacy policy.
    #[inline(always)]
    #[must_use]
    pub const fn error_privacy_policy(&self) -> ErrorPrivacyPolicy {
        self.error_privacy_policy
    }
}

/// Builder for [`LenientJsonDecodeOptions`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LenientJsonDecodeOptionsBuilder {
    options: LenientJsonDecodeOptions,
}

impl LenientJsonDecodeOptionsBuilder {
    /// Creates a builder initialized with the default lenient options.
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self {
            options: LenientJsonDecodeOptions::lenient(),
        }
    }

    /// Configures whether surrounding whitespace is removed.
    #[inline]
    #[must_use]
    pub const fn trim_whitespace(mut self, enabled: bool) -> Self {
        self.options.trim_whitespace = enabled;
        self
    }

    /// Configures whether a leading UTF-8 byte order mark is removed.
    #[inline]
    #[must_use]
    pub const fn strip_utf8_bom(mut self, enabled: bool) -> Self {
        self.options.strip_utf8_bom = enabled;
        self
    }

    /// Configures the policy used to remove one outer Markdown code fence.
    #[inline]
    #[must_use]
    pub const fn markdown_fence_policy(
        mut self,
        policy: MarkdownFencePolicy,
    ) -> Self {
        self.options.markdown_fence_policy = policy;
        self
    }

    /// Configures whether raw control characters in JSON strings are escaped.
    #[inline]
    #[must_use]
    pub const fn escape_control_chars_in_strings(
        mut self,
        enabled: bool,
    ) -> Self {
        self.options.escape_control_chars_in_strings = enabled;
        self
    }

    /// Configures the raw input byte-size limit.
    #[inline]
    #[must_use]
    pub const fn max_input_bytes(mut self, maximum: Option<usize>) -> Self {
        self.options.max_input_bytes = maximum;
        self
    }

    /// Configures the normalized JSON byte-size limit.
    #[inline]
    #[must_use]
    pub const fn max_normalized_bytes(
        mut self,
        maximum: Option<usize>,
    ) -> Self {
        self.options.max_normalized_bytes = maximum;
        self
    }

    /// Configures optional JSON value-resource limits.
    #[inline]
    #[must_use]
    pub const fn value_limits(
        mut self,
        limits: Option<JsonValueLimits>,
    ) -> Self {
        self.options.value_limits = limits;
        self
    }

    /// Configures the privacy policy applied to decoding errors.
    #[inline]
    #[must_use]
    pub const fn error_privacy_policy(
        mut self,
        policy: ErrorPrivacyPolicy,
    ) -> Self {
        self.options.error_privacy_policy = policy;
        self
    }

    /// Builds the configured lenient JSON decode options.
    #[inline]
    #[must_use]
    pub const fn build(self) -> LenientJsonDecodeOptions {
        self.options
    }
}

impl Default for LenientJsonDecodeOptionsBuilder {
    /// Creates a builder initialized with the default lenient options.
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl Default for LenientJsonDecodeOptions {
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
