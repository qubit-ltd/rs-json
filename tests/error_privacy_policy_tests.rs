// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests for [`qubit_json::ErrorPrivacyPolicy`].

use qubit_json::ErrorPrivacyPolicy;

#[test]
fn test_default_is_redacted() {
    assert_eq!(ErrorPrivacyPolicy::default(), ErrorPrivacyPolicy::Redacted,);
}

#[test]
fn test_policy_is_copy_and_equatable() {
    let policy = ErrorPrivacyPolicy::Detailed;
    let copied = policy;
    assert_eq!(policy, copied);
    assert_ne!(policy, ErrorPrivacyPolicy::Redacted);
}
