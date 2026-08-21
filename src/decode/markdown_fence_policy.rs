// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines the lenient Markdown fence normalization policy.

use super::MarkdownFenceClosing;

/// Controls whether and how one outer Markdown code fence is stripped.
///
/// The policy combines fence-language acceptance and closing-fence requirements
/// so callers cannot configure contradictory combinations of independent flags.
///
/// # Examples
///
/// ```
/// use qubit_json::decode::MarkdownFencePolicy;
///
/// let fence_policy = MarkdownFencePolicy::Disabled;
/// assert_eq!(fence_policy, MarkdownFencePolicy::Disabled);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MarkdownFencePolicy {
    /// Leaves Markdown code fences unchanged.
    Disabled,
    /// Strips a fence with any info-string language tag.
    Any {
        /// Controls whether the opening fence needs a matching closing fence.
        closing: MarkdownFenceClosing,
    },
    /// Strips only fences with an empty, `json`, or `jsonc` first info token.
    ///
    /// The `jsonc` token is accepted only as a fence label; fenced content must
    /// still be standard JSON without comments or trailing commas.
    JsonOnly {
        /// Controls whether the opening fence needs a matching closing fence.
        closing: MarkdownFenceClosing,
    },
}
