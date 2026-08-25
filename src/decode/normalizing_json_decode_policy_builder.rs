// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines the builder for [`NormalizingJsonDecodePolicy`].

use super::DiagnosticPolicy;
use super::MarkdownFencePolicy;
use super::NormalizingJsonDecodePolicy;

/// Builder for [`NormalizingJsonDecodePolicy`].
///
/// # Examples
///
/// ```
/// use qubit_json::decode::NormalizingJsonDecodePolicyBuilder;
///
/// let policy = NormalizingJsonDecodePolicyBuilder::new()
///     .strip_utf8_bom(false)
///     .build();
/// assert!(!policy.strip_utf8_bom());
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizingJsonDecodePolicyBuilder {
    /// Policy under construction.
    policy: NormalizingJsonDecodePolicy,
}

impl NormalizingJsonDecodePolicyBuilder {
    /// Creates a builder initialized with the lenient policy.
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self {
            policy: NormalizingJsonDecodePolicy::lenient(),
        }
    }

    /// Configures whether surrounding whitespace is removed.
    #[inline]
    #[must_use]
    pub const fn trim_whitespace(mut self, enabled: bool) -> Self {
        self.policy.set_trim_whitespace(enabled);
        self
    }

    /// Configures whether a leading UTF-8 byte order mark is removed.
    #[inline]
    #[must_use]
    pub const fn strip_utf8_bom(mut self, enabled: bool) -> Self {
        self.policy.set_strip_utf8_bom(enabled);
        self
    }

    /// Configures how one outer Markdown code fence is handled.
    #[inline]
    #[must_use]
    pub const fn markdown_fence_policy(mut self, policy: MarkdownFencePolicy) -> Self {
        self.policy.set_markdown_fence_policy(policy);
        self
    }

    /// Configures whether raw control characters in strings are escaped.
    #[inline]
    #[must_use]
    pub const fn escape_control_chars_in_strings(mut self, enabled: bool) -> Self {
        self.policy.set_escape_control_chars_in_strings(enabled);
        self
    }

    /// Configures the error diagnostic policy.
    #[inline]
    #[must_use]
    pub const fn diagnostic_policy(mut self, policy: DiagnosticPolicy) -> Self {
        self.policy.set_diagnostic_policy(policy);
        self
    }

    /// Builds the configured policy.
    #[inline]
    #[must_use]
    pub const fn build(self) -> NormalizingJsonDecodePolicy {
        self.policy
    }
}

impl Default for NormalizingJsonDecodePolicyBuilder {
    /// Creates a builder initialized with the lenient policy.
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}
