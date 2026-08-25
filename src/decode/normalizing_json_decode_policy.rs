// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines the policy used by the normalizing JSON decoder.

use super::DiagnosticPolicy;
use super::MarkdownFenceClosing;
use super::MarkdownFencePolicy;
use super::NormalizingJsonDecodePolicyBuilder;

/// Text-normalization and diagnostic policy for
/// [`crate::decode::NormalizingJsonDecoder`].
///
/// Resource limits deliberately live in
/// [`qubit_budget::json::JsonDecodeLimits`]
/// and are supplied separately when constructing a decoder.
///
/// # Examples
///
/// ```
/// use qubit_json::decode::NormalizingJsonDecodePolicy;
///
/// let policy = NormalizingJsonDecodePolicy::builder()
///     .trim_whitespace(false)
///     .build();
/// assert!(!policy.trim_whitespace());
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizingJsonDecodePolicy {
    /// Whether leading and trailing whitespace is removed.
    trim_whitespace: bool,
    /// Whether a leading UTF-8 byte order mark is removed.
    strip_utf8_bom: bool,
    /// How one outer Markdown code fence is handled.
    markdown_fence_policy: MarkdownFencePolicy,
    /// Whether raw ASCII control characters in strings are escaped.
    escape_control_chars_in_strings: bool,
    /// Whether input-derived decoding details are retained.
    diagnostic_policy: DiagnosticPolicy,
}

impl NormalizingJsonDecodePolicy {
    /// Creates the default permissive normalization policy.
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
            diagnostic_policy: DiagnosticPolicy::Redacted,
        }
    }

    /// Creates a policy that performs no text rewriting.
    #[inline]
    #[must_use]
    pub const fn strict() -> Self {
        Self {
            trim_whitespace: false,
            strip_utf8_bom: false,
            markdown_fence_policy: MarkdownFencePolicy::Disabled,
            escape_control_chars_in_strings: false,
            diagnostic_policy: DiagnosticPolicy::Redacted,
        }
    }

    /// Creates a builder initialized with the lenient policy.
    #[inline]
    #[must_use]
    pub const fn builder() -> NormalizingJsonDecodePolicyBuilder {
        NormalizingJsonDecodePolicyBuilder::new()
    }

    /// Returns whether surrounding whitespace is removed.
    #[inline(always)]
    #[must_use]
    pub const fn trim_whitespace(&self) -> bool {
        self.trim_whitespace
    }

    /// Returns whether a leading UTF-8 byte order mark is removed.
    #[inline(always)]
    #[must_use]
    pub const fn strip_utf8_bom(&self) -> bool {
        self.strip_utf8_bom
    }

    /// Returns the outer Markdown fence policy.
    #[inline(always)]
    #[must_use]
    pub const fn markdown_fence_policy(&self) -> &MarkdownFencePolicy {
        &self.markdown_fence_policy
    }

    /// Returns whether raw control characters in strings are escaped.
    #[inline(always)]
    #[must_use]
    pub const fn escape_control_chars_in_strings(&self) -> bool {
        self.escape_control_chars_in_strings
    }

    /// Returns the error diagnostic policy.
    #[inline(always)]
    #[must_use]
    pub const fn diagnostic_policy(&self) -> DiagnosticPolicy {
        self.diagnostic_policy
    }

    /// Updates whitespace trimming during builder composition.
    pub(super) const fn set_trim_whitespace(&mut self, enabled: bool) {
        self.trim_whitespace = enabled;
    }

    /// Updates BOM stripping during builder composition.
    pub(super) const fn set_strip_utf8_bom(&mut self, enabled: bool) {
        self.strip_utf8_bom = enabled;
    }

    /// Updates Markdown fence handling during builder composition.
    pub(super) const fn set_markdown_fence_policy(&mut self, policy: MarkdownFencePolicy) {
        self.markdown_fence_policy = policy;
    }

    /// Updates control-character escaping during builder composition.
    pub(super) const fn set_escape_control_chars_in_strings(&mut self, enabled: bool) {
        self.escape_control_chars_in_strings = enabled;
    }

    /// Updates diagnostic handling during builder composition.
    pub(super) const fn set_diagnostic_policy(&mut self, policy: DiagnosticPolicy) {
        self.diagnostic_policy = policy;
    }
}

impl Default for NormalizingJsonDecodePolicy {
    /// Creates the default lenient policy.
    #[inline(always)]
    fn default() -> Self {
        Self::lenient()
    }
}
