/*******************************************************************************
 *
 *    Copyright (c) 2026.
 *    Haixing Hu, Qubit Co. Ltd.
 *
 *    All rights reserved.
 *
 ******************************************************************************/
//! Tests for the public API in `lenient_json_decoder.rs`.
//!
//! Author: Haixing Hu

use serde::Deserialize;
use serde_json::json;

use qubit_json::{JsonDecodeErrorKind, JsonDecodeOptions, JsonTopLevelKind, LenientJsonDecoder};

#[derive(Debug, Deserialize, PartialEq, Eq)]
struct User {
    name: String,
    age: u8,
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
