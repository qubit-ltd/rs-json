// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines the option type used to configure the lenient JSON decoder.

use crate::{
    ErrorPrivacyPolicy,
    MarkdownFenceClosing,
    MarkdownFencePolicy,
};

/// Configuration switches for [`crate::LenientJsonDecoder`].
///
/// Each field controls one normalization rule applied before parsing JSON.
/// Defaults are intentionally conservative and cover the most common
/// non-fully-trusted text inputs without attempting aggressive repair.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    /// size limit is enforced.
    max_input_bytes: Option<usize>,
    /// Controls whether decoding errors retain input-derived serde details.
    error_privacy_policy: ErrorPrivacyPolicy,
}

impl JsonDecodeOptions {
    /// Creates the default lenient option set.
    #[must_use]
    pub const fn lenient() -> Self {
        Self {
            trim_whitespace: true,
            strip_utf8_bom: true,
            markdown_fence_policy: MarkdownFencePolicy::Any {
                closing: MarkdownFenceClosing::Optional,
            },
            escape_control_chars_in_strings: true,
            max_input_bytes: None,
            error_privacy_policy: ErrorPrivacyPolicy::Redacted,
        }
    }

    /// Creates an option set that disables all normalization rules.
    #[must_use]
    pub const fn strict() -> Self {
        Self {
            trim_whitespace: false,
            strip_utf8_bom: false,
            markdown_fence_policy: MarkdownFencePolicy::Disabled,
            escape_control_chars_in_strings: false,
            max_input_bytes: None,
            error_privacy_policy: ErrorPrivacyPolicy::Redacted,
        }
    }

    /// Creates lenient options that only strip JSON-like Markdown code fences.
    #[must_use]
    pub const fn json_code_fences_only() -> Self {
        Self {
            markdown_fence_policy: MarkdownFencePolicy::JsonOnly {
                closing: MarkdownFenceClosing::Optional,
            },
            ..Self::lenient()
        }
    }

    /// Returns whether leading and trailing whitespace is removed.
    #[must_use]
    pub const fn trim_whitespace(&self) -> bool {
        self.trim_whitespace
    }

    /// Returns whether a leading UTF-8 byte order mark is removed.
    #[must_use]
    pub const fn strip_utf8_bom(&self) -> bool {
        self.strip_utf8_bom
    }

    /// Returns the policy used to remove one outer Markdown code fence.
    #[must_use]
    pub const fn markdown_fence_policy(&self) -> MarkdownFencePolicy {
        self.markdown_fence_policy
    }

    /// Returns whether raw control characters in JSON strings are escaped.
    #[must_use]
    pub const fn escape_control_chars_in_strings(&self) -> bool {
        self.escape_control_chars_in_strings
    }

    /// Returns the raw input byte-size limit.
    ///
    /// `Some(limit)` caps accepted input at `limit` bytes. `None` means no
    /// limit is enforced by the decoder.
    #[must_use]
    pub const fn max_input_bytes(&self) -> Option<usize> {
        self.max_input_bytes
    }

    /// Returns the privacy policy applied to decoding error diagnostics.
    #[must_use]
    pub const fn error_privacy_policy(&self) -> ErrorPrivacyPolicy {
        self.error_privacy_policy
    }

    /// Returns a copy with whitespace trimming enabled or disabled.
    #[must_use]
    pub const fn with_trim_whitespace(mut self, enabled: bool) -> Self {
        self.trim_whitespace = enabled;
        self
    }

    /// Returns a copy with UTF-8 byte order mark stripping configured.
    #[must_use]
    pub const fn with_strip_utf8_bom(mut self, enabled: bool) -> Self {
        self.strip_utf8_bom = enabled;
        self
    }

    /// Returns a copy of these options with a Markdown fence policy.
    #[must_use]
    pub const fn with_markdown_fence_policy(
        mut self,
        markdown_fence_policy: MarkdownFencePolicy,
    ) -> Self {
        self.markdown_fence_policy = markdown_fence_policy;
        self
    }

    /// Returns a copy with JSON-string control character escaping configured.
    #[must_use]
    pub const fn with_escape_control_chars_in_strings(
        mut self,
        enabled: bool,
    ) -> Self {
        self.escape_control_chars_in_strings = enabled;
        self
    }

    /// Returns a copy of these options with a raw input byte-size limit.
    #[must_use]
    pub const fn with_max_input_bytes(
        mut self,
        max_input_bytes: Option<usize>,
    ) -> Self {
        self.max_input_bytes = max_input_bytes;
        self
    }

    /// Returns a copy of these options with the requested error privacy policy.
    ///
    /// The policy determines whether serde diagnostics derived from input
    /// values are retained in returned errors.
    #[must_use]
    pub const fn with_error_privacy_policy(
        mut self,
        error_privacy_policy: ErrorPrivacyPolicy,
    ) -> Self {
        self.error_privacy_policy = error_privacy_policy;
        self
    }
}

impl Default for JsonDecodeOptions {
    fn default() -> Self {
        Self::lenient()
    }
}
