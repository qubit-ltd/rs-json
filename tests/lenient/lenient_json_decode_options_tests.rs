// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests for [`qubit_json::decode::NormalizingJsonDecodeOptions`].

use qubit_json::decode::DiagnosticPolicy;
use qubit_json::decode::MarkdownFenceClosing;
use qubit_json::decode::MarkdownFencePolicy;
use qubit_json::decode::NormalizingJsonDecodeOptions;

/// Verifies that default enables all mvp rules.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_default_enables_all_mvp_rules() {
    let options = NormalizingJsonDecodeOptions::default();
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
    assert_eq!(options.diagnostic_policy(), DiagnosticPolicy::Redacted,);
}

/// Verifies that lenient matches default options.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_lenient_matches_default_options() {
    assert_eq!(
        NormalizingJsonDecodeOptions::lenient(),
        NormalizingJsonDecodeOptions::default()
    );
}

/// Verifies that the options builder configures every supported field.
///
/// # Panics
///
/// Panics when the builder does not preserve a configured option.
#[test]
fn test_builder_configures_options_and_consumes_itself() {
    let options = NormalizingJsonDecodeOptions::builder()
        .trim_whitespace(false)
        .strip_utf8_bom(false)
        .markdown_fence_policy(MarkdownFencePolicy::Disabled)
        .escape_control_chars_in_strings(false)
        .max_input_bytes(Some(7))
        .max_normalized_bytes(Some(11))
        .value_limits(None)
        .diagnostic_policy(DiagnosticPolicy::Detailed)
        .build();

    assert!(!options.trim_whitespace());
    assert!(!options.strip_utf8_bom());
    assert_eq!(
        options.markdown_fence_policy(),
        &MarkdownFencePolicy::Disabled,
    );
    assert!(!options.escape_control_chars_in_strings());
    assert_eq!(options.max_input_bytes(), Some(7));
    assert_eq!(options.max_normalized_bytes(), Some(11));
    assert_eq!(options.value_limits(), None);
    assert_eq!(options.diagnostic_policy(), DiagnosticPolicy::Detailed);
}

/// Verifies that strict disables all normalization rules.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_strict_disables_all_normalization_rules() {
    let options = NormalizingJsonDecodeOptions::strict();
    assert!(!options.trim_whitespace());
    assert!(!options.strip_utf8_bom());
    assert_eq!(
        options.markdown_fence_policy(),
        &MarkdownFencePolicy::Disabled,
    );
    assert!(!options.escape_control_chars_in_strings());
    assert_eq!(options.max_input_bytes(), None);
    assert_eq!(options.max_normalized_bytes(), None);
    assert_eq!(options.diagnostic_policy(), DiagnosticPolicy::Redacted,);
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
    let options = NormalizingJsonDecodeOptions::builder()
        .trim_whitespace(true)
        .strip_utf8_bom(true)
        .markdown_fence_policy(markdown_fence_policy.clone())
        .escape_control_chars_in_strings(true)
        .max_input_bytes(Some(64))
        .max_normalized_bytes(Some(128))
        .diagnostic_policy(DiagnosticPolicy::Detailed)
        .build();
    assert!(options.trim_whitespace());
    assert!(options.strip_utf8_bom());
    assert_eq!(options.markdown_fence_policy(), &markdown_fence_policy);
    assert!(options.escape_control_chars_in_strings());
    assert_eq!(options.max_input_bytes(), Some(64));
    assert_eq!(options.max_normalized_bytes(), Some(128));
    assert_eq!(options.diagnostic_policy(), DiagnosticPolicy::Detailed,);
}

/// Verifies that options are cloneable and equatable.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_options_are_clone_and_equatable() {
    let options = NormalizingJsonDecodeOptions::default();
    let cloned = options.clone();
    assert_eq!(options, cloned);
}
