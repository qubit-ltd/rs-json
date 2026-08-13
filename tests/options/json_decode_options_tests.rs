// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests for [`qubit_json::lenient::JsonDecodeOptions`].

use qubit_json::lenient::ErrorPrivacyPolicy;
use qubit_json::lenient::JsonDecodeOptions;
use qubit_json::lenient::MarkdownFenceClosing;
use qubit_json::lenient::MarkdownFencePolicy;

/// Verifies that default enables all mvp rules.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_default_enables_all_mvp_rules() {
    let options = JsonDecodeOptions::default();
    assert!(options.trim_whitespace());
    assert!(options.strip_utf8_bom());
    let markdown_fence_policy: &MarkdownFencePolicy =
        options.markdown_fence_policy();
    assert_eq!(
        markdown_fence_policy,
        &MarkdownFencePolicy::JsonOnly {
            closing: MarkdownFenceClosing::Optional,
        },
    );
    assert!(options.escape_control_chars_in_strings());
    assert_eq!(options.max_input_bytes(), None);
    assert_eq!(options.max_normalized_bytes(), None);
    assert_eq!(options.error_privacy_policy(), ErrorPrivacyPolicy::Redacted,);
}

/// Verifies that lenient matches default options.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_lenient_matches_default_options() {
    assert_eq!(JsonDecodeOptions::lenient(), JsonDecodeOptions::default());
}

/// Verifies that strict disables all normalization rules.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_strict_disables_all_normalization_rules() {
    let options = JsonDecodeOptions::strict();
    assert!(!options.trim_whitespace());
    assert!(!options.strip_utf8_bom());
    assert_eq!(
        options.markdown_fence_policy(),
        &MarkdownFencePolicy::Disabled,
    );
    assert!(!options.escape_control_chars_in_strings());
    assert_eq!(options.max_input_bytes(), None);
    assert_eq!(options.max_normalized_bytes(), None);
    assert_eq!(options.error_privacy_policy(), ErrorPrivacyPolicy::Redacted,);
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
    let options = JsonDecodeOptions::strict()
        .with_trim_whitespace(true)
        .with_strip_utf8_bom(true)
        .with_markdown_fence_policy(markdown_fence_policy.clone())
        .with_escape_control_chars_in_strings(true)
        .with_max_input_bytes(Some(64))
        .with_max_normalized_bytes(Some(128))
        .with_error_privacy_policy(ErrorPrivacyPolicy::Detailed);
    assert!(options.trim_whitespace());
    assert!(options.strip_utf8_bom());
    assert_eq!(options.markdown_fence_policy(), &markdown_fence_policy);
    assert!(options.escape_control_chars_in_strings());
    assert_eq!(options.max_input_bytes(), Some(64));
    assert_eq!(options.max_normalized_bytes(), Some(128));
    assert_eq!(options.error_privacy_policy(), ErrorPrivacyPolicy::Detailed,);
    assert_eq!(
        options.clone().with_max_input_bytes(None).max_input_bytes(),
        None,
    );
    assert_eq!(
        options
            .with_max_normalized_bytes(None)
            .max_normalized_bytes(),
        None,
    );
}

/// Verifies that options are cloneable and equatable.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_options_are_clone_and_equatable() {
    let options = JsonDecodeOptions::default();
    let cloned = options.clone();
    assert_eq!(options, cloned);
}
