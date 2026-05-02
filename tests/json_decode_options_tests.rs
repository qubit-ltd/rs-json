/*******************************************************************************
 *
 *    Copyright (c) 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
//! Tests for [`qubit_json::JsonDecodeOptions`].
//!

use qubit_json::JsonDecodeOptions;

#[test]
fn test_default_enables_all_mvp_rules() {
    let options = JsonDecodeOptions::default();
    assert!(options.trim_whitespace);
    assert!(options.strip_utf8_bom);
    assert!(options.strip_markdown_code_fence);
    assert!(!options.strip_markdown_code_fence_requires_closing);
    assert!(!options.strip_markdown_code_fence_json_only);
    assert!(options.escape_control_chars_in_strings);
    assert_eq!(options.max_input_bytes, None);
}

#[test]
fn test_lenient_matches_default_options() {
    assert_eq!(JsonDecodeOptions::lenient(), JsonDecodeOptions::default());
}

#[test]
fn test_strict_disables_all_normalization_rules() {
    let options = JsonDecodeOptions::strict();
    assert!(!options.trim_whitespace);
    assert!(!options.strip_utf8_bom);
    assert!(!options.strip_markdown_code_fence);
    assert!(!options.strip_markdown_code_fence_requires_closing);
    assert!(!options.strip_markdown_code_fence_json_only);
    assert!(!options.escape_control_chars_in_strings);
    assert_eq!(options.max_input_bytes, None);
}

#[test]
fn test_json_code_fences_only_keeps_lenient_defaults_with_json_tag_restriction() {
    let options = JsonDecodeOptions::json_code_fences_only();
    assert!(options.trim_whitespace);
    assert!(options.strip_utf8_bom);
    assert!(options.strip_markdown_code_fence);
    assert!(!options.strip_markdown_code_fence_requires_closing);
    assert!(options.strip_markdown_code_fence_json_only);
    assert!(options.escape_control_chars_in_strings);
    assert_eq!(options.max_input_bytes, None);
}

#[test]
fn test_with_max_input_bytes_sets_limit_and_preserves_other_options() {
    let options = JsonDecodeOptions::strict().with_max_input_bytes(64);
    assert_eq!(options.max_input_bytes, Some(64));
    assert!(!options.trim_whitespace);
    assert!(!options.strip_markdown_code_fence);
}

#[test]
fn test_options_are_copy_and_equatable() {
    let options = JsonDecodeOptions::default();
    let copied = options;
    assert_eq!(options, copied);
}
