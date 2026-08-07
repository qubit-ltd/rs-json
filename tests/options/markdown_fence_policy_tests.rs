// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests for [`qubit_json::MarkdownFencePolicy`].

use qubit_json::MarkdownFenceClosing;
use qubit_json::MarkdownFencePolicy;

/// Verifies that markdown fence policy is cloneable and equatable.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_markdown_fence_policy_is_clone_and_equatable() {
    let policy = MarkdownFencePolicy::JsonOnly {
        closing: MarkdownFenceClosing::Required,
    };
    let cloned = policy.clone();
    assert_eq!(policy, cloned);
    assert_ne!(policy, MarkdownFencePolicy::Disabled);
}
