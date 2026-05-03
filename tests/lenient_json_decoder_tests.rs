/*******************************************************************************
 *
 *    Copyright (c) 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
//! Tests for the public API in `lenient_json_decoder.rs`.
//!

use serde::Deserialize;
use serde_json::json;

use qubit_json::{
    JsonDecodeErrorKind,
    JsonDecodeOptions,
    JsonTopLevelKind,
    LenientJsonDecoder,
};

#[derive(Debug, Deserialize, PartialEq, Eq)]
struct User {
    name: String,
    age: u8,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
struct Message {
    text: String,
}

#[test]
fn test_new_exposes_configured_options() {
    let options = JsonDecodeOptions {
        strip_markdown_code_fence: false,
        ..JsonDecodeOptions::default()
    };
    let decoder = LenientJsonDecoder::new(options);
    assert_eq!(decoder.options(), &options);
}

#[test]
fn test_default_uses_default_options() {
    let decoder = LenientJsonDecoder::default();
    assert_eq!(decoder.options(), &JsonDecodeOptions::default());
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
fn test_decode_reports_empty_input_from_normalizer() {
    let decoder = LenientJsonDecoder::default();
    let error = decoder
        .decode::<User>("")
        .expect_err("empty input should fail during normalization");
    assert_eq!(error.kind, JsonDecodeErrorKind::EmptyInput);
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
    assert_eq!(error.kind, JsonDecodeErrorKind::UnexpectedTopLevel);
    assert_eq!(error.expected_top_level, Some(JsonTopLevelKind::Object));
    assert_eq!(error.actual_top_level, Some(JsonTopLevelKind::Array));
}

#[test]
fn test_decode_object_reports_empty_input_from_normalizer() {
    let decoder = LenientJsonDecoder::default();
    let error = decoder
        .decode_object::<User>("")
        .expect_err("empty input should fail during normalization");
    assert_eq!(error.kind, JsonDecodeErrorKind::EmptyInput);
}

#[test]
fn test_decode_object_reports_invalid_json_for_malformed_array() {
    let decoder = LenientJsonDecoder::default();
    let error = decoder
        .decode_object::<User>("[")
        .expect_err("malformed JSON should be reported before top-level checking");
    assert_eq!(error.kind, JsonDecodeErrorKind::InvalidJson);
}

#[test]
fn test_decode_object_reports_invalid_json_for_malformed_scalar() {
    let decoder = LenientJsonDecoder::default();
    let error = decoder
        .decode_object::<User>("\"unterminated")
        .expect_err("malformed scalar JSON should not be treated as a top-level mismatch");
    assert_eq!(error.kind, JsonDecodeErrorKind::InvalidJson);
}

#[test]
fn test_decode_array_requires_array_top_level() {
    let decoder = LenientJsonDecoder::default();
    let error = decoder
        .decode_array::<User>("{\"name\":\"alice\",\"age\":30}")
        .expect_err("top-level object should be rejected by decode_array");
    assert_eq!(error.kind, JsonDecodeErrorKind::UnexpectedTopLevel);
    assert_eq!(error.expected_top_level, Some(JsonTopLevelKind::Array));
    assert_eq!(error.actual_top_level, Some(JsonTopLevelKind::Object));
}

#[test]
fn test_decode_array_reports_empty_input_from_normalizer() {
    let decoder = LenientJsonDecoder::default();
    let error = decoder
        .decode_array::<User>("")
        .expect_err("empty input should fail during normalization");
    assert_eq!(error.kind, JsonDecodeErrorKind::EmptyInput);
}

#[test]
fn test_decode_array_reports_invalid_json_for_malformed_object() {
    let decoder = LenientJsonDecoder::default();
    let error = decoder
        .decode_array::<User>("{")
        .expect_err("malformed JSON should be reported before top-level checking");
    assert_eq!(error.kind, JsonDecodeErrorKind::InvalidJson);
}

#[test]
fn test_decode_object_rejects_scalar_top_level() {
    let decoder = LenientJsonDecoder::default();
    let error = decoder
        .decode_object::<User>("42")
        .expect_err("top-level scalar should be rejected by decode_object");
    assert_eq!(error.kind, JsonDecodeErrorKind::UnexpectedTopLevel);
    assert_eq!(error.expected_top_level, Some(JsonTopLevelKind::Object));
    assert_eq!(error.actual_top_level, Some(JsonTopLevelKind::Other));
}

#[test]
fn test_decode_array_rejects_scalar_top_level() {
    let decoder = LenientJsonDecoder::default();
    let error = decoder
        .decode_array::<User>("42")
        .expect_err("top-level scalar should be rejected by decode_array");
    assert_eq!(error.kind, JsonDecodeErrorKind::UnexpectedTopLevel);
    assert_eq!(error.expected_top_level, Some(JsonTopLevelKind::Array));
    assert_eq!(error.actual_top_level, Some(JsonTopLevelKind::Other));
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
        .expect_err("valid object with wrong field type should return Deserialize");
    assert_eq!(error.kind, JsonDecodeErrorKind::Deserialize);
}

#[test]
fn test_decode_array_reports_deserialize_error_after_top_level_check() {
    let decoder = LenientJsonDecoder::default();
    let error = decoder
        .decode_array::<User>("[{\"name\":\"alice\",\"age\":\"old\"}]")
        .expect_err("valid array with wrong element type should return Deserialize");
    assert_eq!(error.kind, JsonDecodeErrorKind::Deserialize);
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
    assert_eq!(error.kind, JsonDecodeErrorKind::InvalidJson);
}

#[test]
fn test_decode_reports_deserialize_error() {
    let decoder = LenientJsonDecoder::default();
    let error = decoder
        .decode::<User>("{\"name\":\"alice\",\"age\":\"old\"}")
        .expect_err("JSON with a wrong field type should return Deserialize");
    assert_eq!(error.kind, JsonDecodeErrorKind::Deserialize);
}

#[test]
fn test_decode_object_reports_invalid_json_for_non_token_start() {
    let decoder = LenientJsonDecoder::new(JsonDecodeOptions {
        trim_whitespace: false,
        strip_markdown_code_fence: false,
        ..JsonDecodeOptions::default()
    });
    let error = decoder
        .decode_object::<User>(" \n\t ")
        .expect_err("invalid syntax should still be mapped as InvalidJson");
    assert_eq!(error.kind, JsonDecodeErrorKind::InvalidJson);
}

#[test]
fn test_normalizer_object_reuses_configuration_between_calls() {
    let decoder = LenientJsonDecoder::new(JsonDecodeOptions {
        strip_markdown_code_fence: false,
        ..JsonDecodeOptions::default()
    });

    let first = decoder.decode_value("```json\n{\"a\":1}\n```");
    assert_eq!(first.unwrap_err().kind, JsonDecodeErrorKind::InvalidJson);

    let second = decoder.decode_value("```json\n{\"a\":2}\n```");
    assert_eq!(second.unwrap_err().kind, JsonDecodeErrorKind::InvalidJson);
}

#[test]
fn test_normalizer_objects_with_different_configs_do_not_share_state() {
    let strict_decoder = LenientJsonDecoder::new(JsonDecodeOptions {
        strip_markdown_code_fence: false,
        ..JsonDecodeOptions::default()
    });
    let permissive_decoder = LenientJsonDecoder::default();

    assert_eq!(
        strict_decoder
            .decode_value("```json\n{\"a\":1}\n```")
            .expect_err("code fence should stay when stripping is disabled")
            .kind,
        JsonDecodeErrorKind::InvalidJson
    );
    let value = permissive_decoder
        .decode_value("```json\n{\"a\":1}\n```")
        .expect("default normalizer should strip one markdown fence");
    assert_eq!(value, serde_json::json!({"a": 1}));
}

#[test]
fn test_normalizer_object_keeps_trim_whitespace_setting_for_empty_text() {
    let decoder = LenientJsonDecoder::new(JsonDecodeOptions {
        trim_whitespace: false,
        ..JsonDecodeOptions::default()
    });
    let error = decoder
        .decode_value(" \n\t")
        .expect_err("trim disabled should leave whitespace for parser");
    assert_eq!(error.kind, JsonDecodeErrorKind::InvalidJson);
}
