// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests for [`qubit_json::decode::DiagnosticPolicy`].

use qubit_json::decode::DiagnosticPolicy;

/// Verifies that default is redacted.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_default_is_redacted() {
    assert_eq!(DiagnosticPolicy::default(), DiagnosticPolicy::Redacted,);
}

/// Verifies that policy is copy and equatable.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_policy_is_copy_and_equatable() {
    let policy = DiagnosticPolicy::Detailed;
    let copied = policy;
    assert_eq!(policy, copied);
    assert_ne!(policy, DiagnosticPolicy::Redacted);
}
