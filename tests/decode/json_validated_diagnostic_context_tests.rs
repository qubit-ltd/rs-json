// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use std::convert::Infallible;

use qubit_json::decode::JsonDecodeContext as Context;
use qubit_json::decode::JsonDecoder;
use qubit_json::decode::JsonNumberPolicy;
use qubit_json::decode::JsonSyntaxError;
use qubit_json::decode::JsonSyntaxErrorReason;
use qubit_json::decode::JsonValidatedDecodeError;
use qubit_json::decode::JsonValidatedDecodeOptions;

#[test]
fn test_diagnostic_context_preserves_eof_stage() {
    for (input, expected) in [
        (b"{\"a\":\"x".as_slice(), Context::String),
        (b"{\"a\"", Context::Colon),
        (b"{\"a\":1", Context::ObjectSeparator),
        (b"{\"a\":[1", Context::ArraySeparator),
        (b"{\"a\":[1   ", Context::ArraySeparator),
        (b"{\"a\":[{\"b\":1   ", Context::ObjectSeparator),
        (b"{\"a\":-", Context::Number),
    ] {
        let error = JsonDecoder::unlimited()
            .decode_unique_validated_utf8(input, &JsonValidatedDecodeOptions::builder().build(), |_, _| {
                Ok::<_, Infallible>(JsonNumberPolicy::Preserve)
            })
            .expect_err("incomplete JSON must fail");
        match error {
            JsonValidatedDecodeError::Decode { error, context } => {
                assert_eq!(context, Some(expected), "input: {input:?}");
                assert_eq!(error.syntax_error().unwrap().context(), Some(expected));
                assert_eq!(
                    error.syntax_error().unwrap().reason(),
                    JsonSyntaxErrorReason::UnexpectedEnd
                );
            }
            other => panic!("unexpected error category for {input:?}: {other:?}"),
        }
    }
}

#[test]
fn test_valid_multibyte_utf8_object_key_is_preserved() {
    let owner = JsonDecoder::unlimited()
        .decode_unique_validated_str(
            r#"{"café":1,"東京":2}"#,
            &JsonValidatedDecodeOptions::builder().build(),
            |_, _| Ok::<_, Infallible>(JsonNumberPolicy::Preserve),
        )
        .expect("valid multibyte UTF-8 keys are accepted");
    assert_eq!(owner.as_value()["café"].as_i64(), Some(1));
    assert_eq!(owner.as_value()["東京"].as_i64(), Some(2));
}

#[test]
fn test_diagnostic_context_tracks_utf8_escapes_and_post_value_positions() {
    for (input, expected, reason) in [
        (
            "{\"é\":\"x\n\"}".as_bytes(),
            Context::String,
            JsonSyntaxErrorReason::UnexpectedByte,
        ),
        (br#"{"a" "#, Context::Colon, JsonSyntaxErrorReason::UnexpectedEnd),
        (
            br#"[1 x]"#,
            Context::ArraySeparator,
            JsonSyntaxErrorReason::ExpectedCommaOrArrayEnd,
        ),
        (
            br#"{"a":1 x}"#,
            Context::ObjectSeparator,
            JsonSyntaxErrorReason::ExpectedCommaOrObjectEnd,
        ),
        (
            br#"true false"#,
            Context::TrailingInput,
            JsonSyntaxErrorReason::TrailingCharacters,
        ),
        (br#"{"a": "#, Context::Value, JsonSyntaxErrorReason::UnexpectedEnd),
        (
            br#"{a:1}"#,
            Context::ObjectKey,
            JsonSyntaxErrorReason::ExpectedObjectKey,
        ),
    ] {
        let error = JsonDecoder::unlimited()
            .decode_unique_validated_utf8(input, &JsonValidatedDecodeOptions::builder().build(), |_, _| {
                Ok::<_, Infallible>(JsonNumberPolicy::Preserve)
            })
            .expect_err("malformed JSON must fail");
        match error {
            JsonValidatedDecodeError::Decode { error, context } => {
                assert_eq!(context, Some(expected), "input: {input:?}");
                let syntax = error.syntax_error().expect("syntax error details");
                assert_eq!(syntax.context(), Some(expected));
                assert_eq!(syntax.reason(), reason, "input: {input:?}");
            }
            other => panic!("unexpected error category for {input:?}: {other:?}"),
        }
    }
}

#[test]
fn test_manually_constructed_syntax_error_has_no_context_until_set() {
    let error = JsonSyntaxError::new(2, 1, 3, JsonSyntaxErrorReason::UnexpectedEnd);
    assert_eq!(error.context(), None);
    let contextual = error.with_context(Context::String);
    assert_eq!(contextual.context(), Some(Context::String));
    assert_eq!(contextual.offset(), error.offset());
    assert_eq!(contextual.line(), error.line());
    assert_eq!(contextual.column(), error.column());
    assert_eq!(contextual.reason(), error.reason());
    assert_eq!(contextual.to_string(), error.to_string());
}
