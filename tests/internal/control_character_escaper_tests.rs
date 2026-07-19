// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests control-character escaping through public decoder behavior.

use serde_json::json;

use qubit_json::{
    JsonDecodeErrorKind,
    JsonDecodeOptions,
    LenientJsonDecoder,
};

/// Verifies that decode value preserves existing escapes.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_value_preserves_existing_escapes() {
    let decoder = LenientJsonDecoder::default();
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
    let decoder = LenientJsonDecoder::default();
    let value = decoder.decode_value("{\"text\":\"a\nb\"}").expect(
        "default decoder should escape control characters inside strings",
    );
    assert_eq!(value, json!({"text": "a\nb"}));
}

/// Verifies that decode value preserves UTF-8 after an escaped control char.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_value_preserves_utf8_after_escaped_control_char() {
    let decoder = LenientJsonDecoder::default();
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
    let decoder = LenientJsonDecoder::new(
        JsonDecodeOptions::default()
            .with_escape_control_chars_in_strings(false),
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

/// Verifies that decode value escapes control char after unmatched backslash.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
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

/// Verifies that decode value escapes control chars after odd and even
/// backslashes.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
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

/// Verifies that decode value leaves non whitespace controls outside strings
/// invalid.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_value_leaves_non_whitespace_controls_outside_strings_invalid() {
    let error = LenientJsonDecoder::default()
        .decode_value("\u{0001}{\"text\":\"value\"}")
        .expect_err("a raw control character outside a JSON string must not be repaired");

    assert_eq!(error.kind(), JsonDecodeErrorKind::InvalidJson);
}
