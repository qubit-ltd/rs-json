// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests for [`qubit_json::decode::MarkdownFenceClosing`].

use qubit_json::decode::MarkdownFenceClosing;

/// Verifies that markdown fence closing is copy and equatable.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_markdown_fence_closing_is_copy_and_equatable() {
    let closing = MarkdownFenceClosing::Required;
    assert_eq!(closing, MarkdownFenceClosing::Required);
    assert_ne!(closing, MarkdownFenceClosing::Optional);
}
