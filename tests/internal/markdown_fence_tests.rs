// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests for Markdown fence normalization behavior.

use qubit_json::JsonDecodeErrorKind;
use qubit_json::JsonDecodeOptions;
use qubit_json::LenientJsonDecoder;
use qubit_json::MarkdownFenceClosing;
use qubit_json::MarkdownFencePolicy;
use serde_json::json;

/// Verifies that decode value default rejects non json markdown fence.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_value_default_rejects_non_json_markdown_fence() {
    let error = LenientJsonDecoder::default()
        .decode_value("~~~python\n{\"ok\":true}\n~~~")
        .expect_err("default decoder must reject a non-JSON fence label");
    assert_eq!(error.kind(), JsonDecodeErrorKind::InvalidJson);
}

/// Verifies that decode value explicit any accepts non json markdown fence.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_value_explicit_any_accepts_non_json_markdown_fence() {
    let decoder = LenientJsonDecoder::new(
        JsonDecodeOptions::default().with_markdown_fence_policy(
            MarkdownFencePolicy::Any {
                closing: MarkdownFenceClosing::Optional,
            },
        ),
    );
    let value = decoder
        .decode_value("~~~python\n{\"ok\":true}\n~~~")
        .expect("explicit Any must preserve the 0.4 compatibility behavior");
    assert_eq!(value, json!({"ok": true}));
}

/// Verifies that decode value strips code fence with closing fence.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_value_strips_code_fence_with_closing_fence() {
    let decoder = LenientJsonDecoder::default();
    let value = decoder
        .decode_value("```json\n{\"a\":1}\n```")
        .expect("default decoder should strip a closing Markdown code fence");
    assert_eq!(value, json!({"a": 1}));
}

/// Verifies that decode value strips code fence with crlf line endings.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_value_strips_code_fence_with_crlf_line_endings() {
    let decoder = LenientJsonDecoder::default();
    let value = decoder
        .decode_value("```json\r\n{\"a\":1}\r\n```")
        .expect("default decoder should accept CRLF fenced JSON");
    assert_eq!(value, json!({"a": 1}));
}

/// Verifies that decode value strips code fence with cr only line endings.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_value_strips_code_fence_with_cr_only_line_endings() {
    let decoder = LenientJsonDecoder::default();
    let value = decoder
        .decode_value("```json\r{\"a\":1}\r```")
        .expect("default decoder should accept CR-only fenced JSON");
    assert_eq!(value, json!({"a": 1}));
}

/// Verifies that decode value strips code fence with mixed line endings lf then
/// cr.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_value_strips_code_fence_with_mixed_line_endings_lf_then_cr() {
    let decoder = LenientJsonDecoder::default();
    let value = decoder
        .decode_value("```json\n{\n\"a\":1\n}\r```")
        .expect("a CR before the closing fence should override earlier LFs");
    assert_eq!(value, json!({"a": 1}));
}

/// Verifies that decode value strips code fence with mixed line endings cr then
/// lf.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_value_strips_code_fence_with_mixed_line_endings_cr_then_lf() {
    let decoder = LenientJsonDecoder::default();
    let value = decoder
        .decode_value("```json\r{\r\"a\":1\r}\n```")
        .expect("an LF before the closing fence should override earlier CRs");
    assert_eq!(value, json!({"a": 1}));
}

/// Verifies that decode value strips tilde code fence.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_value_strips_tilde_code_fence() {
    let decoder = LenientJsonDecoder::default();
    let value = decoder
        .decode_value("~~~json\n{\"a\":1}\n~~~")
        .expect("default decoder should strip a tilde Markdown code fence");
    assert_eq!(value, json!({"a": 1}));
}

/// Verifies that default trimming removes deep indentation before opening-fence
/// recognition.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_value_strips_deeply_indented_opening_fence_after_trimming() {
    let decoder = LenientJsonDecoder::default();
    let value = decoder.decode_value("    ```json\n{\"a\":1}\n```").expect(
        "default trimming should remove opening-fence indentation first",
    );
    assert_eq!(value, json!({"a": 1}));
}

/// Verifies that decode value strips indented code fence when trimming
/// disabled.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_value_strips_indented_code_fence_when_trimming_disabled() {
    let decoder = LenientJsonDecoder::new(
        JsonDecodeOptions::default().with_trim_whitespace(false),
    );
    let value = decoder.decode_value("  ```json\n{\"a\":1}\n  ```").expect(
        "decoder should accept up to three leading spaces before a fence",
    );
    assert_eq!(value, json!({"a": 1}));
}

/// Verifies that decode value rejects deeply indented code fence when trimming
/// disabled.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_value_rejects_deeply_indented_code_fence_when_trimming_disabled()
{
    let decoder = LenientJsonDecoder::new(
        JsonDecodeOptions::default().with_trim_whitespace(false),
    );
    let error = decoder
        .decode_value("    ```json\n{\"a\":1}\n    ```")
        .expect_err("deeply indented fences should remain ordinary text");
    assert_eq!(error.kind(), JsonDecodeErrorKind::InvalidJson);
}

/// Verifies that decode value strips code fence with more than three backticks.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_value_strips_code_fence_with_more_than_three_backticks() {
    let decoder = LenientJsonDecoder::default();
    let value = decoder
        .decode_value("````json\n{\"text\":\"```\"}\n````")
        .expect("decoder should strip matching Markdown fences longer than three backticks");
    assert_eq!(value, json!({"text": "```"}));
}

/// Verifies that decode value strips code fence with longer closing fence.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_value_strips_code_fence_with_longer_closing_fence() {
    let decoder = LenientJsonDecoder::default();
    let value = decoder.decode_value("```json\n{\"a\":1}\n````").expect(
        "decoder should accept a closing fence longer than the opening fence",
    );
    assert_eq!(value, json!({"a": 1}));
}

/// Verifies that decode value strips code fence with indented closing fence.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_value_strips_code_fence_with_indented_closing_fence() {
    let decoder = LenientJsonDecoder::default();
    let value = decoder
        .decode_value("```json\n{\"a\":1}\n   ```   \n")
        .expect("decoder should accept a closing fence alone on a whitespace-padded line");
    assert_eq!(value, json!({"a": 1}));
}

/// Verifies that decode value rejects invalid closing fence indentation with
/// optional policy.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_value_rejects_invalid_closing_fence_indentation_with_optional_policy()
 {
    let decoder = LenientJsonDecoder::default();
    for closing_line in ["    ```", "\t```", "\u{00a0}```"] {
        let input = format!("```json\n{{\"a\":1}}\n{closing_line}");
        let error = decoder.decode_value(&input).expect_err(
            "invalid closing-fence whitespace must remain in the JSON body",
        );
        assert_eq!(error.kind(), JsonDecodeErrorKind::InvalidJson);
    }
}

/// Verifies that decode value rejects invalid closing fence indentation with
/// required policy.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_value_rejects_invalid_closing_fence_indentation_with_required_policy()
 {
    let decoder = LenientJsonDecoder::new(
        JsonDecodeOptions::default().with_markdown_fence_policy(
            MarkdownFencePolicy::Any {
                closing: MarkdownFenceClosing::Required,
            },
        ),
    );
    for closing_line in ["    ```", "\t```", "\u{00a0}```"] {
        let input = format!("```json\n{{\"a\":1}}\n{closing_line}");
        let error = decoder.decode_value(&input).expect_err(
            "required mode must reject invalid closing-fence whitespace",
        );
        assert_eq!(error.kind(), JsonDecodeErrorKind::InvalidJson);
    }
}

/// Verifies that decode value rejects closing fence shorter than opening fence.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_value_rejects_closing_fence_shorter_than_opening_fence() {
    let decoder = LenientJsonDecoder::new(
        JsonDecodeOptions::default().with_markdown_fence_policy(
            MarkdownFencePolicy::Any {
                closing: MarkdownFenceClosing::Required,
            },
        ),
    );
    let error = decoder.decode_value("````json\n{\"a\":1}\n```").expect_err(
        "closing fence shorter than the opening fence should not be stripped",
    );
    assert_eq!(error.kind(), JsonDecodeErrorKind::InvalidJson);
}

/// Verifies that decode value strips code fence without closing fence.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_value_strips_code_fence_without_closing_fence() {
    let decoder = LenientJsonDecoder::default();
    let value = decoder
        .decode_value("```json\n{\"a\":1}")
        .expect("default decoder should strip an opening fence even without a closing fence");
    assert_eq!(value, json!({"a": 1}));
}

/// Verifies that decode value can require closing code fence.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_value_can_require_closing_code_fence() {
    let decoder = LenientJsonDecoder::new(
        JsonDecodeOptions::default().with_markdown_fence_policy(
            MarkdownFencePolicy::Any {
                closing: MarkdownFenceClosing::Required,
            },
        ),
    );
    let error = decoder.decode_value("```json\n{\"a\":1}").expect_err(
        "opening fence without closing fence should be rejected when strict mode is enabled",
    );
    assert_eq!(error.kind(), JsonDecodeErrorKind::InvalidJson);
}

/// Verifies that decode value allows strict closing code fence when present.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_value_allows_strict_closing_code_fence_when_present() {
    let decoder = LenientJsonDecoder::new(
        JsonDecodeOptions::default().with_markdown_fence_policy(
            MarkdownFencePolicy::Any {
                closing: MarkdownFenceClosing::Required,
            },
        ),
    );
    let value = decoder.decode_value("```json\n{\"a\":1}\n```").expect(
        "strict closing mode should still strip a properly closed fence",
    );
    assert_eq!(value, json!({"a": 1}));
}

/// Verifies that decode value can restrict code fence to json language tags.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_value_can_restrict_code_fence_to_json_language_tags() {
    let decoder = LenientJsonDecoder::new(JsonDecodeOptions::lenient());
    let error = decoder
        .decode_value("```python\n{\"a\":1}\n```")
        .expect_err(
            "non-JSON code fence should not be stripped in json-only mode",
        );
    assert_eq!(error.kind(), JsonDecodeErrorKind::InvalidJson);
}

/// Verifies that decode value json only mode accepts longer json code fence.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_value_json_only_mode_accepts_longer_json_code_fence() {
    let decoder = LenientJsonDecoder::new(JsonDecodeOptions::lenient());
    let value = decoder
        .decode_value("````JSON\n{\"a\":1}\n````")
        .expect("json-only mode should accept longer JSON fenced blocks");
    assert_eq!(value, json!({"a": 1}));
}

/// Verifies that decode value json only mode accepts jsonc code fence.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_value_json_only_mode_accepts_jsonc_code_fence() {
    let decoder = LenientJsonDecoder::new(JsonDecodeOptions::lenient());
    let value = decoder
        .decode_value("```jsonc\n{\"a\":1}\n```")
        .expect("json-only mode should accept jsonc fenced blocks");
    assert_eq!(value, json!({"a": 1}));
}

/// Verifies that decode value json only mode accepts empty code fence tag.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_value_json_only_mode_accepts_empty_code_fence_tag() {
    let decoder = LenientJsonDecoder::new(JsonDecodeOptions::lenient());
    let value = decoder.decode_value("```\n{\"a\":1}\n```").expect(
        "json-only mode should accept fenced blocks without a language tag",
    );
    assert_eq!(value, json!({"a": 1}));
}

/// Verifies that decode value json only mode accepts json info string.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_value_json_only_mode_accepts_json_info_string() {
    let decoder = LenientJsonDecoder::new(JsonDecodeOptions::lenient());
    let value = decoder
        .decode_value("```json title=\"sample\"\n{\"a\":1}\n```")
        .expect(
            "json-only mode should accept JSON fenced blocks with info strings",
        );
    assert_eq!(value, json!({"a": 1}));
}

/// Verifies that decode value json only mode rejects non json info string first
/// token.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_value_json_only_mode_rejects_non_json_info_string_first_token() {
    let decoder = LenientJsonDecoder::new(JsonDecodeOptions::lenient());
    let error = decoder
        .decode_value("```python json\n{\"a\":1}\n```")
        .expect_err("json-only mode should use the first info-string token");
    assert_eq!(error.kind(), JsonDecodeErrorKind::InvalidJson);
}

/// Verifies that decode value does not accept inline closing ticks as fence
/// end.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_value_does_not_accept_inline_closing_ticks_as_fence_end() {
    let decoder = LenientJsonDecoder::default();
    let error = decoder.decode_value("```json\n{\"a\":1}```").expect_err(
        "inline trailing ticks are not treated as a valid closing fence",
    );
    assert_eq!(error.kind(), JsonDecodeErrorKind::InvalidJson);
}

/// Verifies that decode value reports invalid json for code fence without
/// newline.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_value_reports_invalid_json_for_code_fence_without_newline() {
    let decoder = LenientJsonDecoder::new(
        JsonDecodeOptions::default().with_trim_whitespace(false),
    );
    let error = decoder
        .decode_value("```json")
        .expect_err("text without a fence body newline should not be stripped");
    assert_eq!(error.kind(), JsonDecodeErrorKind::InvalidJson);
}

/// Verifies that decode value reports empty input for empty code fence body.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_value_reports_empty_input_for_empty_code_fence_body() {
    let decoder = LenientJsonDecoder::default();
    let error = decoder.decode_value("```json\n```").expect_err(
        "empty fenced body should become empty input after normalization",
    );
    assert_eq!(error.kind(), JsonDecodeErrorKind::EmptyInput);
    assert_eq!(error.normalized_input_bytes(), Some(0));
}

/// Verifies that decode value can disable code fence stripping.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_value_can_disable_code_fence_stripping() {
    let decoder = LenientJsonDecoder::new(
        JsonDecodeOptions::default()
            .with_markdown_fence_policy(MarkdownFencePolicy::Disabled),
    );
    let error = decoder
        .decode_value("```json\n{\"name\":\"alice\"}\n```")
        .expect_err("code fences should remain when stripping is disabled");
    assert_eq!(error.kind(), JsonDecodeErrorKind::InvalidJson);
}

/// Verifies that decode value handles uppercase code fence language tag.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_value_handles_uppercase_code_fence_language_tag() {
    let decoder = LenientJsonDecoder::default();
    let value = decoder.decode_value("```JSON\n{\"a\":1}\n```").expect(
        "code fence stripping should not depend on the language tag case",
    );
    assert_eq!(value, json!({"a": 1}));
}
