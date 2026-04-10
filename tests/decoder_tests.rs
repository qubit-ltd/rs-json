/*******************************************************************************
 *
 *    Copyright (c) 2026.
 *    Haixing Hu, Qubit Co. Ltd.
 *
 *    All rights reserved.
 *
 ******************************************************************************/
//! Tests for public decoder behavior in `decoder.rs`.
//!
//! Author: Haixing Hu

use serde::Deserialize;
use serde_json::json;

use qubit_json::{
    JsonDecodeErrorKind, JsonTopLevelKind, LenientJsonDecoder, LenientJsonDecoderOptions,
};

#[derive(Debug, Deserialize, PartialEq, Eq)]
struct User {
    name: String,
    age: u8,
}

#[test]
fn test_new_exposes_configured_options() {
    let options = LenientJsonDecoderOptions {
        strip_markdown_code_fence: false,
        ..LenientJsonDecoderOptions::default()
    };
    let decoder = LenientJsonDecoder::new(options);
    assert_eq!(decoder.options(), &options);
}

#[test]
fn test_default_uses_default_options() {
    let decoder = LenientJsonDecoder::default();
    assert_eq!(decoder.options(), &LenientJsonDecoderOptions::default());
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
