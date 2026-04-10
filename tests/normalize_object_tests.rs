/*******************************************************************************
 *
 *    Copyright (c) 2026.
 *    Haixing Hu, Qubit Co. Ltd.
 *
 *    All rights reserved.
 *
 ******************************************************************************/
//! Tests for `LenientJsonDecoder` as a normalizer object.
//!
//! The tests in this file verify instance-level behavior, especially that
//! normalization options are captured at construction time and reused across
//! multiple decode calls.
//!
//! Author: Haixing Hu

use serde_json::json;

use qubit_json::{JsonDecodeErrorKind, LenientJsonDecoder, LenientJsonDecoderOptions};

#[test]
fn test_normalizer_object_reuses_configuration_between_calls() {
    let mut calls = 0;
    let decoder = LenientJsonDecoder::new(LenientJsonDecoderOptions {
        strip_markdown_code_fence: false,
        ..LenientJsonDecoderOptions::default()
    });

    let first = decoder.decode_value("```json\n{\"a\":1}\n```");
    assert_eq!(first.unwrap_err().kind, JsonDecodeErrorKind::InvalidJson);

    calls += 1;

    let second = decoder.decode_value("```json\n{\"a\":2}\n```");
    assert_eq!(second.unwrap_err().kind, JsonDecodeErrorKind::InvalidJson);

    calls += 1;

    assert_eq!(calls, 2);
}

#[test]
fn test_normalizer_object_with_same_default_config_is_not_sharing_state() {
    let strict_decoder = LenientJsonDecoder::new(LenientJsonDecoderOptions {
        strip_markdown_code_fence: false,
        ..LenientJsonDecoderOptions::default()
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
    assert_eq!(value, json!({"a": 1}));
}

#[test]
fn test_normalizer_object_keeps_trim_whitespace_setting_for_empty_text() {
    let decoder = LenientJsonDecoder::new(LenientJsonDecoderOptions {
        trim_whitespace: false,
        ..LenientJsonDecoderOptions::default()
    });
    let error = decoder
        .decode_value(" \n\t")
        .expect_err("trim disabled should leave whitespace for parser");
    assert_eq!(error.kind, JsonDecodeErrorKind::InvalidJson);
}
