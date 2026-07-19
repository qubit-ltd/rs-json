// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests for [`qubit_json::MarkdownFencePolicy`].

use qubit_json::{
    MarkdownFenceClosing,
    MarkdownFencePolicy,
};

/// Verifies that markdown fence policy is copy and equatable.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_markdown_fence_policy_is_copy_and_equatable() {
    let policy = MarkdownFencePolicy::JsonOnly {
        closing: MarkdownFenceClosing::Required,
    };
    assert_eq!(policy, policy);
    assert_ne!(policy, MarkdownFencePolicy::Disabled);
}
