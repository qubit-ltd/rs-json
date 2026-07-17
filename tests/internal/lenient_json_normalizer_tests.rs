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
fn test_decode_value_trims_surrounding_whitespace_by_default() {
    let decoder = LenientJsonDecoder::default();
    let value = decoder.decode_value("\n{\"text\":\"abc\"}\n").expect(
        "leading and trailing control characters outside strings should be trimmed by default",
    );
    assert_eq!(value, json!({"text": "abc"}));
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
fn test_decode_value_accepts_terminal_unicode_whitespace_when_trimming_enabled()
{
    let decoder = LenientJsonDecoder::default();
    let value = decoder
        .decode_value("```json\n{\"a\":1}\n```\u{00a0}")
        .expect("default trimming should remove terminal Unicode whitespace");
    assert_eq!(value, json!({"a": 1}));
}

#[test]
fn test_decode_value_rejects_terminal_unicode_whitespace_when_trimming_disabled()
 {
    let decoder = LenientJsonDecoder::new(
        JsonDecodeOptions::default().with_trim_whitespace(false),
    );
    let error = decoder
        .decode_value("```json\n{\"a\":1}\n```\u{00a0}")
        .expect_err(
            "terminal Unicode whitespace should remain without trimming",
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
fn test_decode_value_trims_before_control_character_repair() {
    let decoder = LenientJsonDecoder::default();
    let value = decoder
        .decode_value("```json\n  {\"text\":\"a\nb\"}  \n```")
        .expect(
            "outer whitespace should be removed before repair allocates an owned string",
        );
    assert_eq!(value, json!({"text": "a\nb"}));
}
