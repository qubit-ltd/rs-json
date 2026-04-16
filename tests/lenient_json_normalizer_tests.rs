/*******************************************************************************
 *
 *    Copyright (c) 2026.
 *    Haixing Hu, Qubit Co. Ltd.
 *
 *    All rights reserved.
 *
 ******************************************************************************/
//! Tests for normalization behavior implemented in `lenient_json_normalizer.rs`.
//!
//! Author: Haixing Hu

use serde_json::json;

use qubit_json::{JsonDecodeErrorKind, JsonDecodeOptions, LenientJsonDecoder};

#[test]
fn test_decode_value_reports_empty_input_for_empty_string() {
    let decoder = LenientJsonDecoder::default();
    let error = decoder
        .decode_value("")
        .expect_err("empty input should be rejected before JSON parsing");
    assert_eq!(error.kind, JsonDecodeErrorKind::EmptyInput);
}

#[test]
fn test_decode_value_reports_empty_input_for_whitespace_by_default() {
    let decoder = LenientJsonDecoder::default();
    let error = decoder
        .decode_value(" \n\t ")
        .expect_err("whitespace-only input should be empty after default trimming");
    assert_eq!(error.kind, JsonDecodeErrorKind::EmptyInput);
}

#[test]
fn test_decode_value_reports_invalid_json_for_whitespace_when_trimming_disabled() {
    let decoder = LenientJsonDecoder::new(JsonDecodeOptions {
        trim_whitespace: false,
        strip_markdown_code_fence: false,
        ..JsonDecodeOptions::default()
    });
    let error = decoder
        .decode_value("   ")
        .expect_err("whitespace-only input should reach JSON parser when trimming is disabled");
    assert_eq!(error.kind, JsonDecodeErrorKind::InvalidJson);
}

#[test]
fn test_decode_value_respects_input_size_limit() {
    let decoder = LenientJsonDecoder::new(JsonDecodeOptions {
        max_input_bytes: Some(6),
        ..JsonDecodeOptions::default()
    });
    let error = decoder
        .decode_value("{\"a\":1}")
        .expect_err("input above the configured byte limit should be rejected");
    assert_eq!(error.kind, JsonDecodeErrorKind::InputTooLarge);
    assert!(error.to_string().contains("7 bytes"));
}

#[test]
fn test_decode_value_accepts_input_at_size_limit() {
    let decoder = LenientJsonDecoder::new(JsonDecodeOptions {
        max_input_bytes: Some(7),
        ..JsonDecodeOptions::default()
    });
    let value = decoder
        .decode_value("[1,2,3]")
        .expect("input whose size matches the limit should be accepted");
    assert_eq!(value, json!([1, 2, 3]));
}

#[test]
fn test_decode_value_size_limit_runs_before_parser_error_mapping() {
    let decoder = LenientJsonDecoder::new(JsonDecodeOptions {
        max_input_bytes: Some(0),
        ..JsonDecodeOptions::default()
    });
    let error = decoder
        .decode_value("{")
        .expect_err("size guard should run before parser handling");
    assert_eq!(error.kind, JsonDecodeErrorKind::InputTooLarge);
}

#[test]
fn test_decode_value_strips_utf8_bom_by_default() {
    let decoder = LenientJsonDecoder::default();
    let value = decoder
        .decode_value("\u{feff}{\"a\":1}")
        .expect("default decoder should strip a leading UTF-8 BOM");
    assert_eq!(value, json!({"a": 1}));
}

#[test]
fn test_decode_value_reports_empty_input_when_only_bom_is_present() {
    let decoder = LenientJsonDecoder::default();
    let error = decoder
        .decode_value("\u{feff}")
        .expect_err("input containing only BOM should become empty after normalization");
    assert_eq!(error.kind, JsonDecodeErrorKind::EmptyInput);
}

#[test]
fn test_decode_value_can_leave_utf8_bom_when_disabled() {
    let decoder = LenientJsonDecoder::new(JsonDecodeOptions {
        strip_utf8_bom: false,
        ..JsonDecodeOptions::default()
    });
    let error = decoder
        .decode_value("\u{feff}{\"a\":1}")
        .expect_err("BOM should remain and break parsing when BOM stripping is disabled");
    assert_eq!(error.kind, JsonDecodeErrorKind::InvalidJson);
}

#[test]
fn test_decode_value_strips_code_fence_with_closing_fence() {
    let decoder = LenientJsonDecoder::default();
    let value = decoder
        .decode_value("```json\n{\"a\":1}\n```")
        .expect("default decoder should strip a closing Markdown code fence");
    assert_eq!(value, json!({"a": 1}));
}

#[test]
fn test_decode_value_strips_code_fence_without_closing_fence() {
    let decoder = LenientJsonDecoder::default();
    let value = decoder
        .decode_value("```json\n{\"a\":1}")
        .expect("default decoder should strip an opening fence even without a closing fence");
    assert_eq!(value, json!({"a": 1}));
}

#[test]
fn test_decode_value_can_require_closing_code_fence() {
    let decoder = LenientJsonDecoder::new(JsonDecodeOptions {
        strip_markdown_code_fence_requires_closing: true,
        ..JsonDecodeOptions::default()
    });
    let error = decoder.decode_value("```json\n{\"a\":1}").expect_err(
        "opening fence without closing fence should be rejected when strict mode is enabled",
    );
    assert_eq!(error.kind, JsonDecodeErrorKind::InvalidJson);
}

#[test]
fn test_decode_value_allows_strict_closing_code_fence_when_present() {
    let decoder = LenientJsonDecoder::new(JsonDecodeOptions {
        strip_markdown_code_fence_requires_closing: true,
        ..JsonDecodeOptions::default()
    });
    let value = decoder
        .decode_value("```json\n{\"a\":1}\n```")
        .expect("strict closing mode should still strip a properly closed fence");
    assert_eq!(value, json!({"a": 1}));
}

#[test]
fn test_decode_value_can_restrict_code_fence_to_json_language_tags() {
    let decoder = LenientJsonDecoder::new(JsonDecodeOptions {
        strip_markdown_code_fence_json_only: true,
        ..JsonDecodeOptions::default()
    });
    let error = decoder
        .decode_value("```python\n{\"a\":1}\n```")
        .expect_err("non-JSON code fence should not be stripped in json-only mode");
    assert_eq!(error.kind, JsonDecodeErrorKind::InvalidJson);
}

#[test]
fn test_decode_value_json_only_mode_accepts_jsonc_code_fence() {
    let decoder = LenientJsonDecoder::new(JsonDecodeOptions {
        strip_markdown_code_fence_json_only: true,
        ..JsonDecodeOptions::default()
    });
    let value = decoder
        .decode_value("```jsonc\n{\"a\":1}\n```")
        .expect("json-only mode should accept jsonc fenced blocks");
    assert_eq!(value, json!({"a": 1}));
}

#[test]
fn test_decode_value_does_not_accept_inline_closing_ticks_as_fence_end() {
    let decoder = LenientJsonDecoder::default();
    let error = decoder
        .decode_value("```json\n{\"a\":1}```")
        .expect_err("inline trailing ticks are not treated as a valid closing fence");
    assert_eq!(error.kind, JsonDecodeErrorKind::InvalidJson);
}

#[test]
fn test_decode_value_reports_invalid_json_for_code_fence_without_newline() {
    let decoder = LenientJsonDecoder::new(JsonDecodeOptions {
        trim_whitespace: false,
        ..JsonDecodeOptions::default()
    });
    let error = decoder
        .decode_value("```json")
        .expect_err("text without a fence body newline should not be stripped");
    assert_eq!(error.kind, JsonDecodeErrorKind::InvalidJson);
}

#[test]
fn test_decode_value_reports_empty_input_for_empty_code_fence_body() {
    let decoder = LenientJsonDecoder::default();
    let error = decoder
        .decode_value("```json\n```")
        .expect_err("empty fenced body should become empty input after normalization");
    assert_eq!(error.kind, JsonDecodeErrorKind::EmptyInput);
}

#[test]
fn test_decode_value_can_disable_code_fence_stripping() {
    let decoder = LenientJsonDecoder::new(JsonDecodeOptions {
        strip_markdown_code_fence: false,
        ..JsonDecodeOptions::default()
    });
    let error = decoder
        .decode_value("```json\n{\"name\":\"alice\"}\n```")
        .expect_err("code fences should remain when stripping is disabled");
    assert_eq!(error.kind, JsonDecodeErrorKind::InvalidJson);
}

#[test]
fn test_decode_value_preserves_existing_escapes() {
    let decoder = LenientJsonDecoder::default();
    let value = decoder
        .decode_value("{\"text\":\"a\\nb\"}")
        .expect("existing JSON escapes should remain valid");
    assert_eq!(value, json!({"text": "a\nb"}));
}

#[test]
fn test_decode_value_escapes_control_chars_in_strings() {
    let decoder = LenientJsonDecoder::default();
    let value = decoder
        .decode_value("{\"text\":\"a\nb\"}")
        .expect("default decoder should escape control characters inside strings");
    assert_eq!(value, json!({"text": "a\nb"}));
}

#[test]
fn test_decode_value_can_disable_control_char_escaping() {
    let decoder = LenientJsonDecoder::new(JsonDecodeOptions {
        escape_control_chars_in_strings: false,
        ..JsonDecodeOptions::default()
    });
    let error = decoder
        .decode_value("{\"text\":\"a\nb\"}")
        .expect_err("control characters should remain invalid JSON when escaping is disabled");
    assert_eq!(error.kind, JsonDecodeErrorKind::InvalidJson);
}

#[test]
fn test_decode_value_covers_all_supported_control_char_escapes() {
    let control_chars = [
        '\u{0000}', '\u{0001}', '\u{0002}', '\u{0003}', '\u{0004}', '\u{0005}', '\u{0006}',
        '\u{0007}', '\u{0008}', '\u{0009}', '\u{000a}', '\u{000b}', '\u{000c}', '\u{000d}',
        '\u{000e}', '\u{000f}', '\u{0010}', '\u{0011}', '\u{0012}', '\u{0013}', '\u{0014}',
        '\u{0015}', '\u{0016}', '\u{0017}', '\u{0018}', '\u{0019}', '\u{001a}', '\u{001b}',
        '\u{001c}', '\u{001d}', '\u{001e}', '\u{001f}',
    ];
    let control_text: String = control_chars.into_iter().collect();
    let json_input = format!("{{\"text\":\"{control_text}\"}}");

    let decoder = LenientJsonDecoder::default();
    let value = decoder
        .decode_value(&json_input)
        .expect("all supported ASCII control characters should be escaped successfully");
    assert_eq!(value, json!({"text": control_text}));
}

#[test]
fn test_decode_value_trims_surrounding_whitespace_by_default() {
    let decoder = LenientJsonDecoder::default();
    let value = decoder.decode_value("\n{\"text\":\"abc\"}\n").expect(
        "leading and trailing control characters outside strings should be trimmed by default",
    );
    assert_eq!(value, json!({"text": "abc"}));
}

#[test]
fn test_decode_value_with_trim_disabled_and_escape_enabled_still_decodes_owned_output() {
    let decoder = LenientJsonDecoder::new(JsonDecodeOptions {
        trim_whitespace: false,
        strip_markdown_code_fence: false,
        escape_control_chars_in_strings: true,
        ..JsonDecodeOptions::default()
    });
    let value = decoder
        .decode_value("{\"text\":\"a\nb\"}")
        .expect("escaping inside strings should still work when trimming is disabled");
    assert_eq!(value, json!({"text": "a\nb"}));
}

#[test]
fn test_decode_value_trims_owned_output_after_repair() {
    let decoder = LenientJsonDecoder::default();
    let value = decoder
        .decode_value("```json\n  {\"text\":\"a\nb\"}  \n```")
        .expect("trim should still apply after repair allocates an owned string");
    assert_eq!(value, json!({"text": "a\nb"}));
}

#[test]
fn test_decode_value_handles_uppercase_code_fence_language_tag() {
    let decoder = LenientJsonDecoder::default();
    let value = decoder
        .decode_value("```JSON\n{\"a\":1}\n```")
        .expect("code fence stripping should not depend on the language tag case");
    assert_eq!(value, json!({"a": 1}));
}
