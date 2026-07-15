// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines the option type used to configure the lenient JSON decoder.

use crate::{
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
    pub trim_whitespace: bool,
    /// Controls whether a leading UTF-8 byte order mark (`U+FEFF`) is removed
    /// before parsing.
    pub strip_utf8_bom: bool,
    /// Controls whether and how one outer Markdown code fence is removed.
    pub markdown_fence_policy: MarkdownFencePolicy,
    /// Controls whether raw ASCII control characters inside JSON string
    /// literals are converted into valid JSON escape sequences.
    pub escape_control_chars_in_strings: bool,
    /// Caps the accepted raw input size in bytes before normalization.
    ///
    /// When set to `Some(limit)`, any input whose byte length is greater than
    /// `limit` is rejected before further processing. When set to `None`, no
    /// size limit is enforced.
    pub max_input_bytes: Option<usize>,
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

    /// Returns a copy of these options with a Markdown fence policy.
    #[must_use]
    pub const fn with_markdown_fence_policy(
        mut self,
        markdown_fence_policy: MarkdownFencePolicy,
    ) -> Self {
        self.markdown_fence_policy = markdown_fence_policy;
        self
    }

    /// Returns a copy of these options with a raw input byte-size limit.
    #[must_use]
    pub const fn with_max_input_bytes(
        mut self,
        max_input_bytes: usize,
    ) -> Self {
        self.max_input_bytes = Some(max_input_bytes);
        self
    }
}

impl Default for JsonDecodeOptions {
    fn default() -> Self {
        Self::lenient()
    }
}
