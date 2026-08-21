// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests for [`NormalizingJsonDecodePolicy`].

use qubit_json::decode::DiagnosticPolicy;
use qubit_json::decode::MarkdownFenceClosing;
use qubit_json::decode::MarkdownFencePolicy;
use qubit_json::decode::NormalizingJsonDecodePolicy;
use qubit_json::decode::NormalizingJsonDecodePolicyBuilder;

/// Verifies that default enables all mvp rules.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_default_enables_all_mvp_rules() {
    let policy = NormalizingJsonDecodePolicy::default();
    assert!(policy.trim_whitespace());
    assert!(policy.strip_utf8_bom());
    let markdown_fence_policy: &MarkdownFencePolicy = policy.markdown_fence_policy();
    assert_eq!(
        markdown_fence_policy,
        &MarkdownFencePolicy::JsonOnly {
            closing: MarkdownFenceClosing::Optional,
        },
    );
    assert!(policy.escape_control_chars_in_strings());
    assert_eq!(policy.diagnostic_policy(), DiagnosticPolicy::Redacted,);
    assert_eq!(NormalizingJsonDecodePolicyBuilder::default().build(), policy,);
}

/// Verifies that lenient matches default policy.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_lenient_matches_default_policy() {
    assert_eq!(
        NormalizingJsonDecodePolicy::lenient(),
        NormalizingJsonDecodePolicy::default()
    );
}

/// Verifies that the policy builder configures every supported field.
///
/// # Panics
///
/// Panics when the builder does not preserve a configured option.
#[test]
fn test_builder_configures_policy_and_consumes_itself() {
    let policy = NormalizingJsonDecodePolicy::builder()
        .trim_whitespace(false)
        .strip_utf8_bom(false)
        .markdown_fence_policy(MarkdownFencePolicy::Disabled)
        .escape_control_chars_in_strings(false)
        .diagnostic_policy(DiagnosticPolicy::Detailed)
        .build();

    assert!(!policy.trim_whitespace());
    assert!(!policy.strip_utf8_bom());
    assert_eq!(policy.markdown_fence_policy(), &MarkdownFencePolicy::Disabled,);
    assert!(!policy.escape_control_chars_in_strings());
    assert_eq!(policy.diagnostic_policy(), DiagnosticPolicy::Detailed);
}

/// Verifies that strict disables all normalization rules.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_strict_disables_all_normalization_rules() {
    let policy = NormalizingJsonDecodePolicy::strict();
    assert!(!policy.trim_whitespace());
    assert!(!policy.strip_utf8_bom());
    assert_eq!(policy.markdown_fence_policy(), &MarkdownFencePolicy::Disabled,);
    assert!(!policy.escape_control_chars_in_strings());
    assert_eq!(policy.diagnostic_policy(), DiagnosticPolicy::Redacted,);
}

/// Verifies that builders set requested policies.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_builders_set_requested_policies() {
    let markdown_fence_policy = MarkdownFencePolicy::Any {
        closing: MarkdownFenceClosing::Required,
    };
    let policy = NormalizingJsonDecodePolicy::builder()
        .trim_whitespace(true)
        .strip_utf8_bom(true)
        .markdown_fence_policy(markdown_fence_policy.clone())
        .escape_control_chars_in_strings(true)
        .diagnostic_policy(DiagnosticPolicy::Detailed)
        .build();
    assert!(policy.trim_whitespace());
    assert!(policy.strip_utf8_bom());
    assert_eq!(policy.markdown_fence_policy(), &markdown_fence_policy);
    assert!(policy.escape_control_chars_in_strings());
    assert_eq!(policy.diagnostic_policy(), DiagnosticPolicy::Detailed,);
}

/// Verifies that policy are cloneable and equatable.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_policy_are_clone_and_equatable() {
    let policy = NormalizingJsonDecodePolicy::default();
    let cloned = policy.clone();
    assert_eq!(policy, cloned);
}
