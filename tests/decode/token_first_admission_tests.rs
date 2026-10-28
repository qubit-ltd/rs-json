// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use qubit_json::decode::JsonDecodeContext;
use qubit_json::decode::JsonDecoder;
use qubit_json::decode::JsonNumberPolicy;
use qubit_json::decode::JsonSyntaxErrorReason;
use qubit_json::decode::JsonValidatedAdmissionPolicy;
use qubit_json::decode::JsonValidatedDecodeError;
use qubit_json::decode::JsonValidatedDecodeOptions;
use qubit_json::decode::JsonValidatedLimit;

fn options(mode: JsonValidatedAdmissionPolicy, maximum: Option<usize>) -> JsonValidatedDecodeOptions {
    let mut builder = JsonValidatedDecodeOptions::builder().admission_policy(mode);
    if let Some(maximum) = maximum {
        builder = builder.max_encoded_string_bytes(maximum);
    }
    builder.build()
}

fn decode_error(
    text: &str,
    mode: JsonValidatedAdmissionPolicy,
    maximum: Option<usize>,
) -> JsonValidatedDecodeError<()> {
    JsonDecoder::unlimited()
        .decode_unique_validated_str(text, &options(mode, maximum), |_, _| {
            Ok::<_, ()>(JsonNumberPolicy::Preserve)
        })
        .unwrap_err()
}

fn assert_syntax(
    text: &str,
    mode: JsonValidatedAdmissionPolicy,
    maximum: Option<usize>,
    reason: JsonSyntaxErrorReason,
    context: JsonDecodeContext,
    offset: usize,
) {
    let error = decode_error(text, mode, maximum);
    let JsonValidatedDecodeError::Decode { error, context: outer } = error else {
        panic!("expected syntax rejection for {text:?}");
    };
    let syntax = error.syntax_error().expect("decode error must expose syntax details");
    assert_eq!(syntax.reason(), reason, "{text:?}");
    assert_eq!(syntax.context(), Some(context), "{text:?}");
    assert_eq!(outer, syntax.context(), "{text:?}");
    assert_eq!(syntax.offset(), offset, "{text:?}");
}

fn assert_limit(text: &str, maximum: usize, observed: usize, offset: usize) {
    let error = decode_error(text, JsonValidatedAdmissionPolicy::TokenFirst, Some(maximum));
    assert!(
        matches!(
            error,
            JsonValidatedDecodeError::Limit {
                limit: JsonValidatedLimit::EncodedStringBytes,
                maximum: actual_maximum,
                observed: actual_observed,
                offset: actual_offset,
            } if actual_maximum == maximum && actual_observed == observed && actual_offset == offset
        ),
        "unexpected error for {text:?}: {error:?}"
    );
}

#[test]
fn test_token_first_literal_suffix_belongs_to_parent() {
    let token = JsonValidatedAdmissionPolicy::TokenFirst;
    assert_syntax(
        "truex",
        token,
        None,
        JsonSyntaxErrorReason::TrailingCharacters,
        JsonDecodeContext::TrailingInput,
        4,
    );
    assert_syntax(
        "[truex]",
        token,
        None,
        JsonSyntaxErrorReason::ExpectedCommaOrArrayEnd,
        JsonDecodeContext::ArraySeparator,
        5,
    );
    assert_syntax(
        r#"{"a":truex}"#,
        token,
        None,
        JsonSyntaxErrorReason::ExpectedCommaOrObjectEnd,
        JsonDecodeContext::ObjectSeparator,
        9,
    );
    assert_syntax(
        "falsex",
        token,
        None,
        JsonSyntaxErrorReason::TrailingCharacters,
        JsonDecodeContext::TrailingInput,
        5,
    );
    assert_syntax(
        "nullx",
        token,
        None,
        JsonSyntaxErrorReason::TrailingCharacters,
        JsonDecodeContext::TrailingInput,
        4,
    );
    assert_syntax(
        "truex",
        JsonValidatedAdmissionPolicy::Immediate,
        None,
        JsonSyntaxErrorReason::UnexpectedByte,
        JsonDecodeContext::Value,
        4,
    );
    assert_syntax(
        "tru",
        token,
        None,
        JsonSyntaxErrorReason::UnexpectedEnd,
        JsonDecodeContext::Value,
        3,
    );
}

#[test]
fn test_token_first_does_not_call_policy_for_malformed_number_candidates() {
    let options = options(JsonValidatedAdmissionPolicy::TokenFirst, None);
    for text in ["01", "1-2", "1+2", "1..2", "1e+", "1."] {
        let mut calls = 0;
        let error = JsonDecoder::unlimited()
            .decode_unique_validated_str(text, &options, |_, _| {
                calls += 1;
                Ok::<_, ()>(JsonNumberPolicy::Preserve)
            })
            .unwrap_err();
        assert_eq!(calls, 0, "policy called for {text}");
        let JsonValidatedDecodeError::Decode { error, context } = error else {
            panic!("expected number syntax failure for {text}");
        };
        assert_eq!(context, Some(JsonDecodeContext::Number), "{text}");
        let syntax = error.syntax_error().expect("number syntax details");
        assert_eq!(syntax.reason(), JsonSyntaxErrorReason::InvalidNumber, "{text}");
        assert_eq!(syntax.context(), Some(JsonDecodeContext::Number), "{text}");
    }
    for (text, offset) in [("01", 1), ("1-2", 1), ("1+2", 1), ("1..2", 2), ("1e+", 3), ("1.", 2)] {
        assert_syntax(
            text,
            JsonValidatedAdmissionPolicy::TokenFirst,
            None,
            JsonSyntaxErrorReason::InvalidNumber,
            JsonDecodeContext::Number,
            offset,
        );
    }
}

#[test]
fn test_token_first_number_candidate_and_policy_order() {
    let mode = JsonValidatedAdmissionPolicy::TokenFirst;
    for (text, context, reason, offset) in [
        (
            "[1x]",
            JsonDecodeContext::ArraySeparator,
            JsonSyntaxErrorReason::ExpectedCommaOrArrayEnd,
            2,
        ),
        (
            "[01]",
            JsonDecodeContext::Number,
            JsonSyntaxErrorReason::InvalidNumber,
            2,
        ),
        (
            "[1-2]",
            JsonDecodeContext::Number,
            JsonSyntaxErrorReason::InvalidNumber,
            2,
        ),
        (
            r#"{"a":1x}"#,
            JsonDecodeContext::ObjectSeparator,
            JsonSyntaxErrorReason::ExpectedCommaOrObjectEnd,
            6,
        ),
        (
            r#"{"a":01}"#,
            JsonDecodeContext::Number,
            JsonSyntaxErrorReason::InvalidNumber,
            6,
        ),
        (
            r#"{"a":1-2}"#,
            JsonDecodeContext::Number,
            JsonSyntaxErrorReason::InvalidNumber,
            6,
        ),
    ] {
        assert_syntax(text, mode, None, reason, context, offset);
    }
    for text in ["-", "-x", "1e400"] {
        let mut calls = 0;
        let error = JsonDecoder::unlimited()
            .decode_unique_validated_str(text, &options(mode, None), |_, _| {
                calls += 1;
                Ok::<_, ()>(JsonNumberPolicy::Preserve)
            })
            .unwrap_err();
        if text == "1e400" {
            assert_eq!(calls, 1);
            assert!(matches!(error, JsonValidatedDecodeError::Decode { .. }));
        } else {
            assert_eq!(calls, 0);
            assert!(matches!(error, JsonValidatedDecodeError::Decode { .. }));
        }
    }
    assert_syntax(
        "-",
        mode,
        None,
        JsonSyntaxErrorReason::UnexpectedEnd,
        JsonDecodeContext::Number,
        1,
    );
    assert_syntax(
        "-x",
        mode,
        None,
        JsonSyntaxErrorReason::InvalidNumber,
        JsonDecodeContext::Number,
        1,
    );
    assert_syntax(
        "1e400",
        mode,
        None,
        JsonSyntaxErrorReason::FloatOutOfRange,
        JsonDecodeContext::Number,
        0,
    );
    assert_syntax(
        "1x",
        mode,
        None,
        JsonSyntaxErrorReason::TrailingCharacters,
        JsonDecodeContext::TrailingInput,
        1,
    );
    assert_syntax(
        "[1x]",
        JsonValidatedAdmissionPolicy::Immediate,
        None,
        JsonSyntaxErrorReason::ExpectedCommaOrArrayEnd,
        JsonDecodeContext::ArraySeparator,
        2,
    );
}

#[test]
fn test_token_first_string_limit_preserves_exact_eof_boundary() {
    let mode = JsonValidatedAdmissionPolicy::TokenFirst;
    assert_syntax(
        "\"ab",
        mode,
        Some(1),
        JsonSyntaxErrorReason::UnexpectedEnd,
        JsonDecodeContext::String,
        3,
    );
    assert!(
        JsonDecoder::unlimited()
            .decode_unique_validated_str("\"\"", &options(mode, Some(0)), |_, _| {
                Ok::<_, ()>(JsonNumberPolicy::Preserve)
            })
            .is_ok()
    );
    assert_syntax(
        "\"a",
        mode,
        Some(0),
        JsonSyntaxErrorReason::UnexpectedEnd,
        JsonDecodeContext::String,
        2,
    );
    assert_limit("\"a\"", 0, 1, 0);
    assert_limit("\"ab\"", 1, 2, 0);
    assert_limit("\"abc", 1, 2, 0);
    assert_syntax(
        "\"\\q",
        mode,
        None,
        JsonSyntaxErrorReason::UnexpectedEnd,
        JsonDecodeContext::String,
        3,
    );
    assert_syntax(
        "\"\\q\"",
        mode,
        None,
        JsonSyntaxErrorReason::InvalidEscape,
        JsonDecodeContext::String,
        2,
    );
    assert_limit("\"\\q\"", 1, 2, 0);
    assert_syntax(
        "\"abc",
        JsonValidatedAdmissionPolicy::Immediate,
        Some(1),
        JsonSyntaxErrorReason::UnexpectedEnd,
        JsonDecodeContext::String,
        4,
    );
}

#[test]
fn test_token_first_string_boundary_handles_escapes_unicode_and_keys() {
    let mode = JsonValidatedAdmissionPolicy::TokenFirst;
    for text in ["\"\\\"\"", "\"\\\\\"", "\"\\\\\\\"\""] {
        assert!(
            JsonDecoder::unlimited()
                .decode_unique_validated_str(text, &options(mode, None), |_, _| {
                    Ok::<_, ()>(JsonNumberPolicy::Preserve)
                })
                .is_ok(),
            "{text:?}"
        );
    }
    for text in ["\"\\uD83D\\uDE00\"", "\"\\u0061\""] {
        assert!(
            JsonDecoder::unlimited()
                .decode_unique_validated_str(text, &options(mode, None), |_, _| {
                    Ok::<_, ()>(JsonNumberPolicy::Preserve)
                })
                .is_ok(),
            "{text:?}"
        );
    }
    assert_syntax(
        "\"\\uD800\"",
        mode,
        None,
        JsonSyntaxErrorReason::UnpairedSurrogate,
        JsonDecodeContext::String,
        7,
    );
    assert_syntax(
        "\"\\uDC00\"",
        mode,
        None,
        JsonSyntaxErrorReason::UnpairedSurrogate,
        JsonDecodeContext::String,
        7,
    );
    assert_syntax(
        "\"\\u12",
        mode,
        None,
        JsonSyntaxErrorReason::UnexpectedEnd,
        JsonDecodeContext::String,
        5,
    );
    assert_limit("\"中\"", 1, 2, 0);
    assert_limit("\"😀\"", 1, 2, 0);
    assert_limit(r#"{"\q":0}"#, 1, 2, 1);
    assert_syntax(
        r#"{"\q":0}"#,
        mode,
        None,
        JsonSyntaxErrorReason::InvalidEscape,
        JsonDecodeContext::String,
        3,
    );
    let one_node = JsonValidatedDecodeOptions::builder().max_value_and_key_nodes(1).build();
    let error = JsonDecoder::unlimited()
        .decode_unique_validated_str(r#"{"\q":0}"#, &one_node, |_, _| Ok::<_, ()>(JsonNumberPolicy::Preserve))
        .unwrap_err();
    assert!(matches!(
        error,
        JsonValidatedDecodeError::Limit {
            maximum: 1,
            observed: 2,
            offset: 1,
            ..
        }
    ));
}

#[test]
fn test_token_first_and_immediate_agree_for_valid_documents() {
    for text in [
        r#"{"a":[1,2],"b":true,"c":"x\\u0061"}"#,
        "[-0,0.1,18446744073709551615,null,false]",
    ] {
        let mut owners = Vec::new();
        for mode in [
            JsonValidatedAdmissionPolicy::Immediate,
            JsonValidatedAdmissionPolicy::TokenFirst,
        ] {
            let owner = JsonDecoder::unlimited()
                .decode_unique_validated_str(text, &options(mode, None), |_, _| {
                    Ok::<_, ()>(JsonNumberPolicy::Preserve)
                })
                .unwrap_or_else(|error| panic!("valid input failed in {mode:?}: {error:?}"));
            owners.push(owner);
        }
        assert_eq!(owners[0].as_value(), owners[1].as_value(), "{text}");
    }
}
