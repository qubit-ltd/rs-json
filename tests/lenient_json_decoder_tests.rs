// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests for the public API in `lenient_json_decoder.rs`.

use serde_json::json;

use crate::fixtures::{
    ExactInteger,
    Message,
    SingleValue,
    User,
};
use qubit_json::{
    ErrorPrivacyPolicy,
    JsonDecodeErrorKind,
    JsonDecodeOptions,
    JsonDecodeStage,
    JsonTopLevelKind,
    LenientJsonDecoder,
    MarkdownFencePolicy,
};

#[test]
fn test_new_exposes_configured_options() {
    let options = JsonDecodeOptions::default()
        .with_markdown_fence_policy(MarkdownFencePolicy::Disabled);
    let decoder = LenientJsonDecoder::new(options);
    assert_eq!(decoder.options(), &options);
}

#[test]
fn test_default_uses_default_options() {
    let decoder = LenientJsonDecoder::default();
    assert_eq!(decoder.options(), &JsonDecodeOptions::default());
}

#[test]
fn test_strict_decoder_preserves_serde_json_grammar() {
    let decoder = LenientJsonDecoder::new(JsonDecodeOptions::strict());

    let canonical: serde_json::Value = decoder
        .decode(" \n{\"ok\":true}\t")
        .expect("strict mode must preserve whitespace accepted by serde_json");
    assert_eq!(canonical, serde_json::json!({"ok": true}));

    for input in [
        "\u{feff}{\"ok\":true}",
        "```json\n{\"ok\":true}\n```",
        "{\"text\":\"line one\nline two\"}",
    ] {
        let error = decoder
            .decode_value(input)
            .expect_err("strict mode must reject lenient-only input forms");
        assert_eq!(error.kind(), JsonDecodeErrorKind::InvalidJson);
        assert_eq!(
            error.privacy_policy(),
            qubit_json::ErrorPrivacyPolicy::Redacted,
        );
    }
}

#[test]
fn test_decode_value_parses_normalized_json() {
    let decoder = LenientJsonDecoder::default();
    let value = decoder
        .decode_value("```json\n{\"name\":\"alice\",\"age\":30}\n```")
        .expect("default decoder should parse JSON wrapped in a Markdown code fence");
    assert_eq!(value, json!({"name": "alice", "age": 30}));
}

#[test]
fn test_decode_typed_value_succeeds() {
    let decoder = LenientJsonDecoder::default();
    let person: User = decoder
        .decode("{\"name\":\"alice\",\"age\":30}")
        .expect("valid JSON object should deserialize into User");
    assert_eq!(
        person,
        User {
            name: "alice".to_string(),
            age: 30,
        }
    );
}

#[test]
fn test_decode_slice_decodes_valid_utf8_without_changing_semantics() {
    let value: serde_json::Value = LenientJsonDecoder::default()
        .decode_slice(b"{\"ok\":true}")
        .expect("valid UTF-8 JSON bytes must decode");
    assert_eq!(value, serde_json::json!({"ok": true}));
}

#[test]
fn test_decode_slice_accepts_non_rewrite_strict_overrides() {
    let decoder = LenientJsonDecoder::new(
        JsonDecodeOptions::strict()
            .with_max_input_bytes(Some(64))
            .with_error_privacy_policy(ErrorPrivacyPolicy::Detailed),
    );
    let value: serde_json::Value = decoder
        .decode_slice(b"{\"ok\":true}")
        .expect("non-rewrite options must preserve successful byte decoding");
    assert_eq!(value, serde_json::json!({"ok": true}));
}

#[test]
fn test_decode_slice_preserves_deserialize_error_mapping() {
    let decoder = LenientJsonDecoder::new(
        JsonDecodeOptions::strict()
            .with_error_privacy_policy(ErrorPrivacyPolicy::Detailed),
    );
    let error = decoder.decode_slice::<Message>(b"{\"text\":7}").expect_err(
        "valid JSON with the wrong field type must fail deserialization",
    );
    assert_eq!(error.kind(), JsonDecodeErrorKind::Deserialize);
    assert_eq!(error.privacy_policy(), ErrorPrivacyPolicy::Detailed);
    assert!(std::error::Error::source(&error).is_some());
}

#[test]
fn test_decode_slice_preserves_invalid_json_mapping() {
    let decoder = LenientJsonDecoder::new(JsonDecodeOptions::strict());
    let error = decoder
        .decode_slice::<Message>(b"{\"text\":\"broken\"")
        .expect_err("malformed typed JSON must remain an invalid JSON error");
    assert_eq!(error.kind(), JsonDecodeErrorKind::InvalidJson);
}

#[test]
fn test_decode_slice_checks_raw_size_before_utf8() {
    let decoder = LenientJsonDecoder::new(
        JsonDecodeOptions::strict().with_max_input_bytes(Some(1)),
    );
    let error = decoder
        .decode_slice::<serde_json::Value>(&[0xff, 0xfe])
        .expect_err("raw size must be checked before UTF-8");
    assert_eq!(error.kind(), JsonDecodeErrorKind::InputTooLarge);
}

#[test]
fn test_decode_slice_classifies_invalid_utf8() {
    let error = LenientJsonDecoder::default()
        .decode_slice::<serde_json::Value>(&[0xff])
        .expect_err("invalid UTF-8 must fail before normalization");
    assert_eq!(error.kind(), JsonDecodeErrorKind::InvalidUtf8);
    assert_eq!(error.stage(), JsonDecodeStage::DecodeText);
    assert_eq!(error.raw_input_bytes(), 1);
    assert_eq!(error.normalized_input_bytes(), None);
}

#[test]
fn test_decode_reports_empty_input_from_normalizer() {
    let decoder = LenientJsonDecoder::default();
    let error = decoder
        .decode::<User>("")
        .expect_err("empty input should fail during normalization");
    assert_eq!(error.kind(), JsonDecodeErrorKind::EmptyInput);
}

#[test]
fn test_decode_typed_value_applies_normalization_pipeline() {
    let decoder = LenientJsonDecoder::default();
    let message: Message = decoder
        .decode("```json\n{\"text\":\"a\nb\"}\n```")
        .expect("typed decode should still normalize fenced JSON and repair string control chars");
    assert_eq!(
        message,
        Message {
            text: "a\nb".to_string(),
        }
    );
}

#[test]
fn test_decode_object_requires_object_top_level() {
    let decoder = LenientJsonDecoder::default();
    let error = decoder
        .decode_object::<User>("[{\"name\":\"alice\",\"age\":30}]")
        .expect_err("top-level array should be rejected by decode_object");
    assert_eq!(error.kind(), JsonDecodeErrorKind::UnexpectedTopLevel);
    assert_eq!(error.expected_top_level(), Some(JsonTopLevelKind::Object));
    assert_eq!(error.actual_top_level(), Some(JsonTopLevelKind::Array));
}

#[test]
fn test_decode_object_reports_empty_input_from_normalizer() {
    let decoder = LenientJsonDecoder::default();
    let error = decoder
        .decode_object::<User>("")
        .expect_err("empty input should fail during normalization");
    assert_eq!(error.kind(), JsonDecodeErrorKind::EmptyInput);
}

#[test]
fn test_decode_object_reports_invalid_json_for_malformed_array() {
    let decoder = LenientJsonDecoder::default();
    let error = decoder.decode_object::<User>("[").expect_err(
        "malformed JSON should be reported before top-level checking",
    );
    assert_eq!(error.kind(), JsonDecodeErrorKind::InvalidJson);
}

#[test]
fn test_decode_object_reports_invalid_json_for_malformed_scalar() {
    let decoder = LenientJsonDecoder::default();
    let error = decoder.decode_object::<User>("\"unterminated").expect_err(
        "malformed scalar JSON should not be treated as a top-level mismatch",
    );
    assert_eq!(error.kind(), JsonDecodeErrorKind::InvalidJson);
}

#[test]
fn test_decode_array_requires_array_top_level() {
    let decoder = LenientJsonDecoder::default();
    let error = decoder
        .decode_array::<User>("{\"name\":\"alice\",\"age\":30}")
        .expect_err("top-level object should be rejected by decode_array");
    assert_eq!(error.kind(), JsonDecodeErrorKind::UnexpectedTopLevel);
    assert_eq!(error.expected_top_level(), Some(JsonTopLevelKind::Array));
    assert_eq!(error.actual_top_level(), Some(JsonTopLevelKind::Object));
}

#[test]
fn test_decode_array_reports_empty_input_from_normalizer() {
    let decoder = LenientJsonDecoder::default();
    let error = decoder
        .decode_array::<User>("")
        .expect_err("empty input should fail during normalization");
    assert_eq!(error.kind(), JsonDecodeErrorKind::EmptyInput);
}

#[test]
fn test_decode_array_reports_invalid_json_for_malformed_object() {
    let decoder = LenientJsonDecoder::default();
    let error = decoder.decode_array::<User>("{").expect_err(
        "malformed JSON should be reported before top-level checking",
    );
    assert_eq!(error.kind(), JsonDecodeErrorKind::InvalidJson);
}

#[test]
fn test_decode_object_rejects_scalar_top_level() {
    let decoder = LenientJsonDecoder::default();
    let error = decoder
        .decode_object::<User>("42")
        .expect_err("top-level scalar should be rejected by decode_object");
    assert_eq!(error.kind(), JsonDecodeErrorKind::UnexpectedTopLevel);
    assert_eq!(error.expected_top_level(), Some(JsonTopLevelKind::Object));
    assert_eq!(error.actual_top_level(), Some(JsonTopLevelKind::Other));
}

#[test]
fn test_decode_array_rejects_scalar_top_level() {
    let decoder = LenientJsonDecoder::default();
    let error = decoder
        .decode_array::<User>("42")
        .expect_err("top-level scalar should be rejected by decode_array");
    assert_eq!(error.kind(), JsonDecodeErrorKind::UnexpectedTopLevel);
    assert_eq!(error.expected_top_level(), Some(JsonTopLevelKind::Array));
    assert_eq!(error.actual_top_level(), Some(JsonTopLevelKind::Other));
}

#[test]
fn test_decode_array_succeeds() {
    let decoder = LenientJsonDecoder::default();
    let people = decoder
        .decode_array::<User>("[{\"name\":\"alice\",\"age\":30}]")
        .expect("top-level array should deserialize into Vec<User>");
    assert_eq!(
        people,
        vec![User {
            name: "alice".to_string(),
            age: 30,
        }]
    );
}

#[test]
fn test_decode_object_reports_deserialize_error_after_top_level_check() {
    let decoder = LenientJsonDecoder::default();
    let error = decoder
        .decode_object::<User>("{\"name\":\"alice\",\"age\":\"old\"}")
        .expect_err(
            "valid object with wrong field type should return Deserialize",
        );
    assert_eq!(error.kind(), JsonDecodeErrorKind::Deserialize);
}

#[test]
fn test_decode_array_reports_deserialize_error_after_top_level_check() {
    let decoder = LenientJsonDecoder::default();
    let error = decoder
        .decode_array::<User>("[{\"name\":\"alice\",\"age\":\"old\"}]")
        .expect_err(
            "valid array with wrong element type should return Deserialize",
        );
    assert_eq!(error.kind(), JsonDecodeErrorKind::Deserialize);
}

#[test]
fn test_decode_allows_generic_scalar_targets() {
    let decoder = LenientJsonDecoder::default();
    let value: i64 = decoder
        .decode("42")
        .expect("scalar JSON should deserialize into i64");
    assert_eq!(value, 42);
}

#[test]
fn test_decode_reports_invalid_json() {
    let decoder = LenientJsonDecoder::default();
    let error = decoder
        .decode::<User>("{")
        .expect_err("broken JSON should return InvalidJson");
    assert_eq!(error.kind(), JsonDecodeErrorKind::InvalidJson);
}

#[test]
fn test_decode_reports_deserialize_error() {
    let decoder = LenientJsonDecoder::default();
    let error = decoder
        .decode::<User>("{\"name\":\"alice\",\"age\":\"old\"}")
        .expect_err("JSON with a wrong field type should return Deserialize");
    assert_eq!(error.kind(), JsonDecodeErrorKind::Deserialize);
}

#[test]
fn test_decode_reports_invalid_json_when_data_error_precedes_syntax_error() {
    let error = LenientJsonDecoder::default()
        .decode::<SingleValue>("{\"value\":\"wrong\",")
        .expect_err(
            "incomplete JSON must take precedence over a field type error",
        );

    assert_eq!(error.kind(), JsonDecodeErrorKind::InvalidJson);
    assert_eq!(error.stage(), JsonDecodeStage::Parse);
}

#[test]
fn test_decode_object_reports_invalid_json_when_data_error_precedes_syntax_error()
 {
    let error = LenientJsonDecoder::default()
        .decode_object::<SingleValue>("{\"value\":\"wrong\",")
        .expect_err("incomplete object JSON must take precedence over a field type error");

    assert_eq!(error.kind(), JsonDecodeErrorKind::InvalidJson);
    assert_eq!(error.stage(), JsonDecodeStage::Parse);
}

#[test]
fn test_decode_array_reports_invalid_json_when_data_error_precedes_syntax_error()
 {
    let error = LenientJsonDecoder::default()
        .decode_array::<u8>("[\"wrong\",")
        .expect_err("incomplete array JSON must take precedence over an element type error");

    assert_eq!(error.kind(), JsonDecodeErrorKind::InvalidJson);
    assert_eq!(error.stage(), JsonDecodeStage::Parse);
}

#[test]
fn test_decode_object_reports_invalid_json_for_non_token_start() {
    let decoder = LenientJsonDecoder::new(
        JsonDecodeOptions::default()
            .with_trim_whitespace(false)
            .with_markdown_fence_policy(MarkdownFencePolicy::Disabled),
    );
    let error = decoder
        .decode_object::<User>(" \n\t ")
        .expect_err("invalid syntax should still be mapped as InvalidJson");
    assert_eq!(error.kind(), JsonDecodeErrorKind::InvalidJson);
}

#[test]
fn test_normalizer_object_reuses_configuration_between_calls() {
    let decoder = LenientJsonDecoder::new(
        JsonDecodeOptions::default()
            .with_markdown_fence_policy(MarkdownFencePolicy::Disabled),
    );

    let first = decoder.decode_value("```json\n{\"a\":1}\n```");
    assert_eq!(first.unwrap_err().kind(), JsonDecodeErrorKind::InvalidJson);

    let second = decoder.decode_value("```json\n{\"a\":2}\n```");
    assert_eq!(second.unwrap_err().kind(), JsonDecodeErrorKind::InvalidJson);
}

#[test]
fn test_normalizer_objects_with_different_configs_do_not_share_state() {
    let strict_decoder = LenientJsonDecoder::new(
        JsonDecodeOptions::default()
            .with_markdown_fence_policy(MarkdownFencePolicy::Disabled),
    );
    let permissive_decoder = LenientJsonDecoder::default();

    assert_eq!(
        strict_decoder
            .decode_value("```json\n{\"a\":1}\n```")
            .expect_err("code fence should stay when stripping is disabled")
            .kind(),
        JsonDecodeErrorKind::InvalidJson
    );
    let value = permissive_decoder
        .decode_value("```json\n{\"a\":1}\n```")
        .expect("default normalizer should strip one markdown fence");
    assert_eq!(value, serde_json::json!({"a": 1}));
}

#[test]
fn test_normalizer_object_keeps_trim_whitespace_setting_for_empty_text() {
    let decoder = LenientJsonDecoder::new(
        JsonDecodeOptions::default().with_trim_whitespace(false),
    );
    let error = decoder
        .decode_value(" \n\t")
        .expect_err("trim disabled should leave whitespace for parser");
    assert_eq!(error.kind(), JsonDecodeErrorKind::InvalidJson);
}

#[test]
fn test_decode_object_preserves_u128_without_value_round_trip() {
    let decoded: ExactInteger = LenientJsonDecoder::default()
        .decode_object(r#"{"value":340282366920938463463374607431768211455}"#)
        .expect("direct object decoding should preserve u128::MAX");

    assert_eq!(decoded.value, u128::MAX);
}

#[test]
fn test_decode_array_preserves_u128_without_value_round_trip() {
    let decoded: Vec<ExactInteger> = LenientJsonDecoder::default()
        .decode_array(r#"[{"value":340282366920938463463374607431768211455}]"#)
        .expect("direct array decoding should preserve u128::MAX");

    assert_eq!(decoded, vec![ExactInteger { value: u128::MAX }]);
}

#[test]
fn test_decode_object_preserves_duplicate_field_rejection() {
    let error = LenientJsonDecoder::default()
        .decode_object::<SingleValue>(r#"{"value":1,"value":2}"#)
        .expect_err("direct object decoding should reject duplicate fields");

    assert_eq!(error.kind(), JsonDecodeErrorKind::Deserialize);
}
