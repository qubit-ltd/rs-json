// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests for [`qubit_json::MarkdownFenceClosing`].

use qubit_json::MarkdownFenceClosing;

#[test]
fn test_markdown_fence_closing_is_copy_and_equatable() {
    let closing = MarkdownFenceClosing::Required;
    assert_eq!(closing, MarkdownFenceClosing::Required);
    assert_ne!(closing, MarkdownFenceClosing::Optional);
}
