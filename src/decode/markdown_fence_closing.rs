// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines the closing-fence requirement for lenient Markdown fence
//! normalization.

/// Specifies whether an opening Markdown code fence needs a closing fence.
///
/// # Examples
///
/// ```
/// use qubit_json::decode::MarkdownFenceClosing;
///
/// let closing_policy = MarkdownFenceClosing::Optional;
/// assert_eq!(closing_policy, MarkdownFenceClosing::Optional);
/// ```
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MarkdownFenceClosing {
    /// Accepts an opening fence even when no matching closing fence is present.
    Optional,
    /// Requires a matching closing fence before stripping the outer fence.
    Required,
}
