// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests control-character escaping through public lenient decoder behavior.

use qubit_budget::json::JsonDecodeLimits;
use qubit_json::decode::JsonDecodeError;
use qubit_json::decode::JsonDecodeErrorKind;
use qubit_json::decode::JsonDecodeStage;
use qubit_json::decode::NormalizingJsonDecodePolicy;
use qubit_json::decode::NormalizingJsonDecoder;
use serde_json::json;

/// Returns the configured limit retained by a measured budget failure.
fn configured_limit(error: &JsonDecodeError) -> usize {
    error
        .budget_error()
        .and_then(|error| error.budget_error())
        .expect("the normalized-size failure must contain a budget error")
        .configured_limit()
}

/// Verifies that decode value preserves existing escapes.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_value_preserves_existing_escapes() {
    let mut decoder =
        NormalizingJsonDecoder::with_limits(NormalizingJsonDecodePolicy::default(), qubit_budget::json::JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::default());
    let value = decoder
        .decode_value("{\"text\":\"a\\nb\"}")
        .expect("existing JSON escapes should remain valid");
    assert_eq!(value, json!({"text": "a\nb"}));
}

/// Verifies that decode value escapes control chars in strings.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_value_escapes_control_chars_in_strings() {
    let mut decoder =
        NormalizingJsonDecoder::with_limits(NormalizingJsonDecodePolicy::default(), qubit_budget::json::JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::default());
    let value = decoder
        .decode_value("{\"text\":\"a\nb\"}")
        .expect("default decoder should escape control characters inside strings");
    assert_eq!(value, json!({"text": "a\nb"}));
}

/// Verifies that decode value preserves UTF-8 after an escaped control char.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_value_preserves_utf8_after_escaped_control_char() {
    let mut decoder =
        NormalizingJsonDecoder::with_limits(NormalizingJsonDecodePolicy::default(), qubit_budget::json::JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::default());
    let value = decoder
        .decode_value("{\"text\":\"first\n你好😀\nlast\\nend\"}")
        .expect("control escaping should preserve following UTF-8 text");
    assert_eq!(value, json!({"text": "first\n你好😀\nlast\nend"}));
}

/// Verifies that decode value can disable control char escaping.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_value_can_disable_control_char_escaping() {
    let mut decoder = NormalizingJsonDecoder::with_limits(
        NormalizingJsonDecodePolicy::builder()
            .escape_control_chars_in_strings(false)
            .build(),
        qubit_budget::json::JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::default(),
    );
    let error = decoder
        .decode_value("{\"text\":\"a\nb\"}")
        .expect_err("control characters should remain invalid JSON when escaping is disabled");
    assert_eq!(error.kind(), JsonDecodeErrorKind::InvalidJson);
}

/// Verifies that decode value covers all supported control char escapes.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_value_covers_all_supported_control_char_escapes() {
    let control_chars = [
        '\u{0000}', '\u{0001}', '\u{0002}', '\u{0003}', '\u{0004}', '\u{0005}', '\u{0006}', '\u{0007}', '\u{0008}',
        '\u{0009}', '\u{000a}', '\u{000b}', '\u{000c}', '\u{000d}', '\u{000e}', '\u{000f}', '\u{0010}', '\u{0011}',
        '\u{0012}', '\u{0013}', '\u{0014}', '\u{0015}', '\u{0016}', '\u{0017}', '\u{0018}', '\u{0019}', '\u{001a}',
        '\u{001b}', '\u{001c}', '\u{001d}', '\u{001e}', '\u{001f}',
    ];
    let control_text: String = control_chars.into_iter().collect();
    let json_input = format!("{{\"text\":\"{control_text}\"}}");

    let mut decoder =
        NormalizingJsonDecoder::with_limits(NormalizingJsonDecodePolicy::default(), qubit_budget::json::JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::default());
    let value = decoder
        .decode_value(&json_input)
        .expect("all supported ASCII control characters should be escaped successfully");
    assert_eq!(value, json!({"text": control_text}));
}

/// Verifies that every C0 byte is repaired at every chunk boundary offset.
///
/// # Panics
///
/// Panics when a control byte is not repaired at the expected offset.
#[test]
fn test_decode_value_escapes_each_control_char_at_each_chunk_offset() {
    let mut decoder =
        NormalizingJsonDecoder::with_limits(NormalizingJsonDecodePolicy::default(), qubit_budget::json::JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::default());

    for prefix_len in 0..=24 {
        for control_code in 0_u8..=0x1f {
            let control = char::from(control_code);
            let prefix = "a".repeat(prefix_len);
            let json_input = format!("{{\"text\":\"{prefix}{control}suffix\"}}");
            let decoded = decoder.decode_value(&json_input).unwrap_or_else(|error| {
                panic!(
                    "U+{control_code:04X} at payload offset {prefix_len} \
                     should be repaired: {error}"
                )
            });
            assert_eq!(
                decoded,
                json!({"text": format!("{prefix}{control}suffix")}),
                "unexpected result for U+{control_code:04X} at payload offset {prefix_len}",
            );
        }
    }
}

/// Verifies string and escape state across every chunk boundary offset.
///
/// # Panics
///
/// Panics when legal outer whitespace changes string state, an escaped quote
/// closes the string, or a raw string newline is not repaired.
#[test]
fn test_decode_value_preserves_state_across_each_chunk_boundary_offset() {
    for prefix_len in 0..=24 {
        let prefix = "a".repeat(prefix_len);
        let mut json_input = format!("\n{{\n  \"text\":\"{prefix}escaped quote: \\\"; escaped slash: \\\\; ",);
        json_input.push('\n');
        json_input.push_str("tail\"\n}\r\n");
        let expected = json!({
            "text": format!(
                "{prefix}escaped quote: \"; escaped slash: \\; \ntail",
            ),
        });

        let decoded =
            NormalizingJsonDecoder::with_limits(NormalizingJsonDecodePolicy::default(), qubit_budget::json::JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::default())
                .decode_value(&json_input)
                .unwrap_or_else(|error| {
                    panic!(
                        "state transitions at prefix offset {prefix_len} should \
                     decode: {error}"
                    )
                });
        assert_eq!(decoded, expected);

        let bounded = NormalizingJsonDecoder::with_limits(
            NormalizingJsonDecodePolicy::builder().build(),
            JsonDecodeLimits::builder()
                .max_normalized_input_bytes(json_input.len() + 1)
                .build(),
        )
        .decode_value(&json_input)
        .unwrap_or_else(|error| {
            panic!(
                "bounded state transitions at prefix offset {prefix_len} \
                 should decode: {error}"
            )
        });
        assert_eq!(bounded, expected);
    }
}

/// Verifies that the normalized-size limit accounts for every C0 escape.
///
/// # Panics
///
/// Panics when the configured boundary or error metadata is not observed.
#[test]
fn test_decode_value_bounds_all_control_character_escapes_by_normalized_size() {
    let control_chars: String = (0_u8..=0x1f).map(char::from).collect();
    let json_input = format!("{{\"text\":\"{control_chars}\"}}");
    let normalized_bytes = "{\"text\":\"".len() + (5 * 2) + (27 * 6) + "\"}".len();

    let accepted = NormalizingJsonDecoder::with_limits(
        NormalizingJsonDecodePolicy::builder().build(),
        JsonDecodeLimits::builder()
            .max_normalized_input_bytes(normalized_bytes)
            .build(),
    )
    .decode_value(&json_input)
    .expect("all C0 replacements should fit exactly at their normalized limit");
    assert_eq!(accepted, json!({"text": control_chars}));

    let error = NormalizingJsonDecoder::with_limits(
        NormalizingJsonDecodePolicy::builder().build(),
        JsonDecodeLimits::builder()
            .max_normalized_input_bytes(normalized_bytes - 1)
            .build(),
    )
    .decode_value(&json_input)
    .expect_err("one byte below the C0 replacement size must fail");
    assert_eq!(error.kind(), JsonDecodeErrorKind::Budget);
    assert_eq!(error.stage(), JsonDecodeStage::Normalize);
    assert_eq!(error.normalized_input_bytes(), Some(normalized_bytes));
    assert_eq!(configured_limit(&error), normalized_bytes - 1);
}

/// Verifies that decode value escapes control char after unmatched backslash.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_value_escapes_control_char_after_unmatched_backslash() {
    let mut decoder =
        NormalizingJsonDecoder::with_limits(NormalizingJsonDecodePolicy::default(), qubit_budget::json::JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::default());

    for code_point in 0_u32..=0x1f {
        let control = char::from_u32(code_point).expect("ASCII control code points should be valid chars");
        let mut json_input = String::from("{\"text\":\"");
        json_input.push('\\');
        json_input.push(control);
        json_input.push_str("\"}");

        let value = decoder.decode_value(&json_input).unwrap_or_else(|error| {
            panic!("control U+{code_point:04X} after an unmatched backslash should be repaired: {error}")
        });
        assert_eq!(
            value,
            json!({"text": control.to_string()}),
            "unexpected decoded value for U+{code_point:04X}",
        );
    }
}

/// Verifies that an equal-length unmatched-backslash repair still occurs when
/// a normalized-size limit is configured.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_value_repairs_equal_length_escape_at_normalized_size_limit() {
    let json_input = "{\"text\":\"\\\n\"}";
    let mut decoder = NormalizingJsonDecoder::with_limits(
        NormalizingJsonDecodePolicy::builder().build(),
        JsonDecodeLimits::builder()
            .max_normalized_input_bytes(json_input.len())
            .build(),
    );

    let value = decoder
        .decode_value(json_input)
        .expect("equal-length repair should still run with a normalized-size limit");
    assert_eq!(value, json!({"text": "\n"}));
}

/// Verifies that decode value escapes control chars after odd and even
/// backslashes.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_value_escapes_control_chars_after_odd_and_even_backslashes() {
    let mut decoder =
        NormalizingJsonDecoder::with_limits(NormalizingJsonDecodePolicy::default(), qubit_budget::json::JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::default());

    for control in ['\n', '\u{0000}'] {
        for backslash_count in 1..=4 {
            let mut json_input = String::from("{\"text\":\"");
            json_input.extend(std::iter::repeat_n('\\', backslash_count));
            json_input.push(control);
            json_input.push_str("\"}");

            let value = decoder.decode_value(&json_input).unwrap_or_else(|error| {
                panic!("{backslash_count} backslashes before {control:?} should be repaired: {error}")
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

/// Verifies that decode value leaves non whitespace controls outside strings
/// invalid.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_value_leaves_non_whitespace_controls_outside_strings_invalid() {
    let error = NormalizingJsonDecoder::with_limits(NormalizingJsonDecodePolicy::default(), qubit_budget::json::JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::default())
        .decode_value("\u{0001}{\"text\":\"value\"}")
        .expect_err("a raw control character outside a JSON string must not be repaired");

    assert_eq!(error.kind(), JsonDecodeErrorKind::InvalidJson);
}
