// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines the Markdown fence normalization policy.

use crate::MarkdownFenceClosing;

/// Controls whether and how one outer Markdown code fence is stripped.
///
/// The policy combines fence-language acceptance and closing-fence requirements
/// so callers cannot configure contradictory combinations of independent flags.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MarkdownFencePolicy {
    /// Leaves Markdown code fences unchanged.
    Disabled,
    /// Strips a fence with any info-string language tag.
    Any {
        /// Controls whether the opening fence needs a matching closing fence.
        closing: MarkdownFenceClosing,
    },
    /// Strips only fences with an empty, `json`, or `jsonc` first info token.
    JsonOnly {
        /// Controls whether the opening fence needs a matching closing fence.
        closing: MarkdownFenceClosing,
    },
}
