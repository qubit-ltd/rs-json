// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests for normalization behavior implemented in
//! `lenient_json_normalizer.rs`.

use serde_json::json;

use qubit_json::{
    JsonDecodeErrorKind,
    JsonDecodeOptions,
    JsonDecodeStage,
    LenientJsonDecoder,
    MarkdownFenceClosing,
    MarkdownFencePolicy,
};

#[test]
fn test_decode_value_reports_empty_input_for_empty_string() {
    let decoder = LenientJsonDecoder::default();
    let error = decoder
        .decode_value("")
        .expect_err("empty input should be rejected before JSON parsing");
    assert_eq!(error.kind(), JsonDecodeErrorKind::EmptyInput);
    assert_eq!(error.normalized_input_bytes(), None);
}

#[test]
fn test_decode_value_reports_empty_input_for_whitespace_by_default() {
    let decoder = LenientJsonDecoder::default();
    let error = decoder.decode_value(" \n\t ").expect_err(
        "whitespace-only input should be empty after default trimming",
    );
    assert_eq!(error.kind(), JsonDecodeErrorKind::EmptyInput);
    assert_eq!(error.normalized_input_bytes(), None);
}

#[test]
fn test_decode_value_reports_invalid_json_for_whitespace_when_trimming_disabled()
 {
    let decoder = LenientJsonDecoder::new(
        JsonDecodeOptions::default()
            .with_trim_whitespace(false)
            .with_markdown_fence_policy(MarkdownFencePolicy::Disabled),
    );
    let error = decoder
        .decode_value("   ")
        .expect_err("whitespace-only input should reach JSON parser when trimming is disabled");
    assert_eq!(error.kind(), JsonDecodeErrorKind::InvalidJson);
}

#[test]
fn test_decode_value_respects_input_size_limit() {
    let decoder = LenientJsonDecoder::new(
        JsonDecodeOptions::default().with_max_input_bytes(Some(6)),
    );
    let error = decoder
        .decode_value("{\"a\":1}")
        .expect_err("input above the configured byte limit should be rejected");
    assert_eq!(error.kind(), JsonDecodeErrorKind::InputTooLarge);
    assert_eq!(error.stage(), JsonDecodeStage::Normalize);
    assert_eq!(error.raw_input_bytes(), 7);
    assert_eq!(error.max_input_bytes(), Some(6));
    assert!(error.to_string().contains("6 bytes"));
}

#[test]
fn test_decode_value_accepts_input_at_size_limit() {
    let decoder = LenientJsonDecoder::new(
        JsonDecodeOptions::default().with_max_input_bytes(Some(7)),
    );
    let value = decoder
        .decode_value("[1,2,3]")
        .expect("input whose size matches the limit should be accepted");
    assert_eq!(value, json!([1, 2, 3]));
}

#[test]
fn test_decode_value_size_limit_runs_before_parser_error_mapping() {
    let decoder = LenientJsonDecoder::new(
        JsonDecodeOptions::default().with_max_input_bytes(Some(0)),
    );
    let error = decoder
        .decode_value("{")
        .expect_err("size guard should run before parser handling");
    assert_eq!(error.kind(), JsonDecodeErrorKind::InputTooLarge);
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
    let error = decoder.decode_value("\u{feff}").expect_err(
        "input containing only BOM should become empty after normalization",
    );
    assert_eq!(error.kind(), JsonDecodeErrorKind::EmptyInput);
    assert_eq!(error.normalized_input_bytes(), Some(0));
}

#[test]
fn test_decode_value_can_leave_utf8_bom_when_disabled() {
    let decoder = LenientJsonDecoder::new(
        JsonDecodeOptions::default().with_strip_utf8_bom(false),
    );
    let error = decoder.decode_value("\u{feff}{\"a\":1}").expect_err(
        "BOM should remain and break parsing when BOM stripping is disabled",
    );
    assert_eq!(error.kind(), JsonDecodeErrorKind::InvalidJson);
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
fn test_decode_value_strips_code_fence_with_mixed_line_endings_lf_then_cr() {
    let decoder = LenientJsonDecoder::default();
    let value = decoder
        .decode_value("```json\n{\n\"a\":1\n}\r```")
        .expect("a CR before the closing fence should override earlier LFs");
    assert_eq!(value, json!({"a": 1}));
}

#[test]
fn test_decode_value_strips_code_fence_with_mixed_line_endings_cr_then_lf() {
    let decoder = LenientJsonDecoder::default();
    let value = decoder
        .decode_value("```json\r{\r\"a\":1\r}\n```")
        .expect("an LF before the closing fence should override earlier CRs");
    assert_eq!(value, json!({"a": 1}));
}

#[test]
fn test_decode_value_strips_tilde_code_fence() {
    let decoder = LenientJsonDecoder::default();
    let value = decoder
        .decode_value("~~~json\n{\"a\":1}\n~~~")
        .expect("default decoder should strip a tilde Markdown code fence");
    assert_eq!(value, json!({"a": 1}));
}

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

#[test]
fn test_decode_value_strips_code_fence_with_more_than_three_backticks() {
    let decoder = LenientJsonDecoder::default();
    let value = decoder
        .decode_value("````json\n{\"text\":\"```\"}\n````")
        .expect("decoder should strip matching Markdown fences longer than three backticks");
    assert_eq!(value, json!({"text": "```"}));
}

#[test]
fn test_decode_value_strips_code_fence_with_longer_closing_fence() {
    let decoder = LenientJsonDecoder::default();
    let value = decoder.decode_value("```json\n{\"a\":1}\n````").expect(
        "decoder should accept a closing fence longer than the opening fence",
    );
    assert_eq!(value, json!({"a": 1}));
}

#[test]
fn test_decode_value_strips_code_fence_with_indented_closing_fence() {
    let decoder = LenientJsonDecoder::default();
    let value = decoder
        .decode_value("```json\n{\"a\":1}\n   ```   \n")
        .expect("decoder should accept a closing fence alone on a whitespace-padded line");
    assert_eq!(value, json!({"a": 1}));
}

#[test]
fn test_decode_value_rejects_invalid_closing_fence_indentation_with_optional_policy()
{
    let decoder = LenientJsonDecoder::default();
    for closing_line in [
        "    ```",
        "\t```",
        "\u{00a0}```",
    ] {
        let input = format!("```json\n{{\"a\":1}}\n{closing_line}");
        let error = decoder.decode_value(&input).expect_err(
            "invalid closing-fence whitespace must remain in the JSON body",
        );
        assert_eq!(error.kind(), JsonDecodeErrorKind::InvalidJson);
    }
}

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
    for closing_line in [
        "    ```",
        "\t```",
        "\u{00a0}```",
    ] {
        let input = format!("```json\n{{\"a\":1}}\n{closing_line}");
        let error = decoder.decode_value(&input).expect_err(
            "required mode must reject invalid closing-fence whitespace",
        );
        assert_eq!(error.kind(), JsonDecodeErrorKind::InvalidJson);
    }
}

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

#[test]
fn test_decode_value_can_restrict_code_fence_to_json_language_tags() {
    let decoder =
        LenientJsonDecoder::new(JsonDecodeOptions::json_code_fences_only());
    let error = decoder
        .decode_value("```python\n{\"a\":1}\n```")
        .expect_err(
            "non-JSON code fence should not be stripped in json-only mode",
        );
    assert_eq!(error.kind(), JsonDecodeErrorKind::InvalidJson);
}

#[test]
fn test_decode_value_json_only_mode_accepts_longer_json_code_fence() {
    let decoder =
        LenientJsonDecoder::new(JsonDecodeOptions::json_code_fences_only());
    let value = decoder
        .decode_value("````JSON\n{\"a\":1}\n````")
        .expect("json-only mode should accept longer JSON fenced blocks");
    assert_eq!(value, json!({"a": 1}));
}

#[test]
fn test_decode_value_json_only_mode_accepts_jsonc_code_fence() {
    let decoder =
        LenientJsonDecoder::new(JsonDecodeOptions::json_code_fences_only());
    let value = decoder
        .decode_value("```jsonc\n{\"a\":1}\n```")
        .expect("json-only mode should accept jsonc fenced blocks");
    assert_eq!(value, json!({"a": 1}));
}

#[test]
fn test_decode_value_json_only_mode_accepts_empty_code_fence_tag() {
    let decoder =
        LenientJsonDecoder::new(JsonDecodeOptions::json_code_fences_only());
    let value = decoder.decode_value("```\n{\"a\":1}\n```").expect(
        "json-only mode should accept fenced blocks without a language tag",
    );
    assert_eq!(value, json!({"a": 1}));
}

#[test]
fn test_decode_value_json_only_mode_accepts_json_info_string() {
    let decoder =
        LenientJsonDecoder::new(JsonDecodeOptions::json_code_fences_only());
    let value = decoder
        .decode_value("```json title=\"sample\"\n{\"a\":1}\n```")
        .expect(
            "json-only mode should accept JSON fenced blocks with info strings",
        );
    assert_eq!(value, json!({"a": 1}));
}

#[test]
fn test_decode_value_json_only_mode_rejects_non_json_info_string_first_token() {
    let decoder =
        LenientJsonDecoder::new(JsonDecodeOptions::json_code_fences_only());
    let error = decoder
        .decode_value("```python json\n{\"a\":1}\n```")
        .expect_err("json-only mode should use the first info-string token");
    assert_eq!(error.kind(), JsonDecodeErrorKind::InvalidJson);
}

#[test]
fn test_decode_value_does_not_accept_inline_closing_ticks_as_fence_end() {
    let decoder = LenientJsonDecoder::default();
    let error = decoder.decode_value("```json\n{\"a\":1}```").expect_err(
        "inline trailing ticks are not treated as a valid closing fence",
    );
    assert_eq!(error.kind(), JsonDecodeErrorKind::InvalidJson);
}

#[test]
fn test_decode_value_randomized_inputs_do_not_panic_and_round_trip_when_valid()
{
    let decoders = [
        LenientJsonDecoder::default(),
        LenientJsonDecoder::new(
            JsonDecodeOptions::default()
                .with_trim_whitespace(false)
                .with_markdown_fence_policy(MarkdownFencePolicy::Disabled),
        ),
        LenientJsonDecoder::new(
            JsonDecodeOptions::default().with_markdown_fence_policy(
                MarkdownFencePolicy::JsonOnly {
                    closing: MarkdownFenceClosing::Required,
                },
            ),
        ),
    ];

    let mut seed = 0x0d15_ea5e_d5e0_ded5u64;
    for _ in 0..3000 {
        let input = generate_noisy_json_candidate(&mut seed);
        for decoder in &decoders {
            let result =
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    decoder.decode_value(&input)
                }));
            assert!(result.is_ok(), "decoder panicked on input: {input:?}");

            if let Ok(value) = result.expect("catch_unwind returned no result")
            {
                let canonical = serde_json::to_string(&value)
                    .expect("serializing a decoded JSON value should not fail");
                let reparsed = decoder.decode_value(&canonical).expect(
                    "canonical JSON should be decodable by the same decoder",
                );
                assert_eq!(reparsed, value);
            }
        }
    }
}

fn generate_noisy_json_candidate(seed: &mut u64) -> String {
    const ALPHABET: &[char] = &[
        '{', '}', '[', ']', ':', ',', '"', '\\', '`', ' ', '\t', '\n', '\r',
        'a', 'b', 'c', 'x', 'y', 'z', '0', '1', '2', '9', '-', '.', 't', 'f',
        'n', '\u{0000}', '\u{0008}', '\u{001f}',
    ];

    let len = (next_u64(seed) % 48) as usize;
    let mut text = String::with_capacity(len + 16);
    for _ in 0..len {
        let index = (next_u64(seed) % ALPHABET.len() as u64) as usize;
        text.push(ALPHABET[index]);
    }

    match next_u64(seed) % 4 {
        0 => format!("```json\n{text}\n```"),
        1 => format!("```python\n{text}\n```"),
        2 => format!("\u{feff}{text}"),
        _ => text,
    }
}

fn next_u64(seed: &mut u64) -> u64 {
    *seed = seed
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    *seed
}

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

#[test]
fn test_decode_value_reports_empty_input_for_empty_code_fence_body() {
    let decoder = LenientJsonDecoder::default();
    let error = decoder.decode_value("```json\n```").expect_err(
        "empty fenced body should become empty input after normalization",
    );
    assert_eq!(error.kind(), JsonDecodeErrorKind::EmptyInput);
    assert_eq!(error.normalized_input_bytes(), Some(0));
}

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
    let value = decoder.decode_value("{\"text\":\"a\nb\"}").expect(
        "default decoder should escape control characters inside strings",
    );
    assert_eq!(value, json!({"text": "a\nb"}));
}

#[test]
fn test_decode_value_can_disable_control_char_escaping() {
    let decoder = LenientJsonDecoder::new(
        JsonDecodeOptions::default()
            .with_escape_control_chars_in_strings(false),
    );
    let error = decoder
        .decode_value("{\"text\":\"a\nb\"}")
        .expect_err("control characters should remain invalid JSON when escaping is disabled");
    assert_eq!(error.kind(), JsonDecodeErrorKind::InvalidJson);
}

#[test]
fn test_decode_value_covers_all_supported_control_char_escapes() {
    let control_chars = [
        '\u{0000}', '\u{0001}', '\u{0002}', '\u{0003}', '\u{0004}', '\u{0005}',
        '\u{0006}', '\u{0007}', '\u{0008}', '\u{0009}', '\u{000a}', '\u{000b}',
        '\u{000c}', '\u{000d}', '\u{000e}', '\u{000f}', '\u{0010}', '\u{0011}',
        '\u{0012}', '\u{0013}', '\u{0014}', '\u{0015}', '\u{0016}', '\u{0017}',
        '\u{0018}', '\u{0019}', '\u{001a}', '\u{001b}', '\u{001c}', '\u{001d}',
        '\u{001e}', '\u{001f}',
    ];
    let control_text: String = control_chars.into_iter().collect();
    let json_input = format!("{{\"text\":\"{control_text}\"}}");

    let decoder = LenientJsonDecoder::default();
    let value = decoder.decode_value(&json_input).expect(
        "all supported ASCII control characters should be escaped successfully",
    );
    assert_eq!(value, json!({"text": control_text}));
}

#[test]
fn test_decode_value_escapes_control_char_after_unmatched_backslash() {
    let decoder = LenientJsonDecoder::default();

    for code_point in 0_u32..=0x1f {
        let control = char::from_u32(code_point)
            .expect("ASCII control code points should be valid chars");
        let mut json_input = String::from("{\"text\":\"");
        json_input.push('\\');
        json_input.push(control);
        json_input.push_str("\"}");

        let value = decoder.decode_value(&json_input).unwrap_or_else(|error| {
            panic!(
                "control U+{code_point:04X} after an unmatched backslash should be repaired: {error}"
            )
        });
        assert_eq!(
            value,
            json!({"text": control.to_string()}),
            "unexpected decoded value for U+{code_point:04X}",
        );
    }
}

#[test]
fn test_decode_value_escapes_control_chars_after_odd_and_even_backslashes() {
    let decoder = LenientJsonDecoder::default();

    for control in ['\n', '\u{0000}'] {
        for backslash_count in 1..=4 {
            let mut json_input = String::from("{\"text\":\"");
            json_input.extend(std::iter::repeat_n('\\', backslash_count));
            json_input.push(control);
            json_input.push_str("\"}");

            let value = decoder.decode_value(&json_input).unwrap_or_else(|error| {
                panic!(
                    "{backslash_count} backslashes before {control:?} should be repaired: {error}"
                )
            });
            let mut expected = "\\".repeat(backslash_count / 2);
            expected.push(control);
            assert_eq!(
                value,
                json!({"text": expected}),
                "unexpected decoded value for {backslash_count} backslashes before {control:?}",
            );
        }
    }
}

#[test]
fn test_decode_value_leaves_non_whitespace_controls_outside_strings_invalid() {
    let error = LenientJsonDecoder::default()
        .decode_value("\u{0001}{\"text\":\"value\"}")
        .expect_err("a raw control character outside a JSON string must not be repaired");

    assert_eq!(error.kind(), JsonDecodeErrorKind::InvalidJson);
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
fn test_decode_value_with_trim_disabled_and_escape_enabled_still_decodes_owned_output()
 {
    let decoder = LenientJsonDecoder::new(
        JsonDecodeOptions::default()
            .with_trim_whitespace(false)
            .with_markdown_fence_policy(MarkdownFencePolicy::Disabled)
            .with_escape_control_chars_in_strings(true),
    );
    let value = decoder.decode_value("{\"text\":\"a\nb\"}").expect(
        "escaping inside strings should still work when trimming is disabled",
    );
    assert_eq!(value, json!({"text": "a\nb"}));
}

#[test]
fn test_decode_value_trims_owned_output_after_repair() {
    let decoder = LenientJsonDecoder::default();
    let value = decoder
        .decode_value("```json\n  {\"text\":\"a\nb\"}  \n```")
        .expect(
            "trim should still apply after repair allocates an owned string",
        );
    assert_eq!(value, json!({"text": "a\nb"}));
}

#[test]
fn test_decode_value_handles_uppercase_code_fence_language_tag() {
    let decoder = LenientJsonDecoder::default();
    let value = decoder.decode_value("```JSON\n{\"a\":1}\n```").expect(
        "code fence stripping should not depend on the language tag case",
    );
    assert_eq!(value, json!({"a": 1}));
}
