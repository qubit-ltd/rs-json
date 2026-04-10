/*******************************************************************************
 *
 *    Copyright (c) 2026.
 *    Haixing Hu, Qubit Co. Ltd.
 *
 *    All rights reserved.
 *
 ******************************************************************************/
//! Tests for [`qubit_json::LenientJsonDecoderOptions`].
//!
//! Author: Haixing Hu

use qubit_json::LenientJsonDecoderOptions;

#[test]
fn test_default_enables_all_mvp_rules() {
    let options = LenientJsonDecoderOptions::default();
    assert!(options.trim_whitespace);
    assert!(options.strip_utf8_bom);
    assert!(options.strip_markdown_code_fence);
    assert!(options.escape_control_chars_in_strings);
}

#[test]
fn test_options_are_copy_and_equatable() {
    let options = LenientJsonDecoderOptions::default();
    let copied = options;
    assert_eq!(options, copied);
}
