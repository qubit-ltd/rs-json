use std::convert::Infallible;

use qubit_budget::ResourceLimit;
use qubit_budget::StructureLimits;
use qubit_budget::json::JsonDecodeLimits;
use qubit_budget::json::JsonResource;
use qubit_budget::json::JsonValueLimits;

use crate::decode::JsonDecodeContext as Context;
use crate::decode::JsonDecodeErrorKind;
use crate::decode::JsonDecoder;
use crate::decode::JsonMaterializationErrorReason;
use crate::decode::JsonNumberPolicy;
use crate::decode::JsonNumberToken;
use crate::decode::JsonPathSegment;
use crate::decode::JsonRootKind;
use crate::decode::JsonValidatedAdmissionPolicy;
use crate::decode::JsonValidatedDecodeError;
use crate::decode::JsonValidatedDecodeOptions;
use crate::decode::JsonValidatedLimit;

fn options(policy: JsonValidatedAdmissionPolicy) -> JsonValidatedDecodeOptions {
    JsonValidatedDecodeOptions::builder().admission_policy(policy).build()
}

fn decode_error(input: &str, policy: JsonValidatedAdmissionPolicy) -> JsonValidatedDecodeError<Infallible> {
    JsonDecoder::unlimited()
        .decode_unique_validated_str(input, &options(policy), |_, _| {
            Ok::<_, Infallible>(JsonNumberPolicy::Preserve)
        })
        .expect_err("malformed JSON must fail")
}

#[test]
fn immediate_remains_the_default_and_keeps_its_first_error_order() {
    let defaults = JsonValidatedDecodeOptions::builder().build();
    assert_eq!(defaults.admission_policy(), JsonValidatedAdmissionPolicy::Immediate);
    assert_eq!(
        JsonValidatedDecodeOptions::builder().build().admission_policy(),
        JsonValidatedAdmissionPolicy::Immediate
    );

    match decode_error("{\"a\":truex}", JsonValidatedAdmissionPolicy::Immediate) {
        JsonValidatedDecodeError::Decode { error, context } => {
            assert_eq!(error.kind(), JsonDecodeErrorKind::InvalidJson);
            assert_eq!(context, Some(Context::Value));
            assert_eq!(error.syntax_error().unwrap().context(), context);
        }
        other => panic!("unexpected error: {other:?}"),
    }
    match decode_error("1-2", JsonValidatedAdmissionPolicy::Immediate) {
        JsonValidatedDecodeError::Decode { error, context } => {
            assert_eq!(context, Some(Context::TrailingInput));
            assert_eq!(
                error.syntax_error().unwrap().reason(),
                crate::decode::JsonSyntaxErrorReason::TrailingCharacters
            );
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn token_first_admits_complete_literals_before_container_delimiters() {
    for (input, expected) in [
        ("truex", Context::TrailingInput),
        ("[truex]", Context::ArraySeparator),
        ("{\"a\":truex}", Context::ObjectSeparator),
        ("{\"a\":falsex}", Context::ObjectSeparator),
        ("{\"a\":nullx}", Context::ObjectSeparator),
    ] {
        match decode_error(input, JsonValidatedAdmissionPolicy::TokenFirst) {
            JsonValidatedDecodeError::Decode { error, context } => {
                assert_eq!(error.kind(), JsonDecodeErrorKind::InvalidJson, "{input}");
                assert_eq!(context, Some(expected), "{input}");
                assert_eq!(error.syntax_error().unwrap().context(), context, "{input}");
            }
            other => panic!("unexpected error for {input}: {other:?}"),
        }
    }

    for (input, reason) in [
        ("tru", crate::decode::JsonSyntaxErrorReason::UnexpectedEnd),
        ("[tru]", crate::decode::JsonSyntaxErrorReason::UnexpectedByte),
    ] {
        match decode_error(input, JsonValidatedAdmissionPolicy::TokenFirst) {
            JsonValidatedDecodeError::Decode { error, context } => {
                assert_eq!(context, Some(Context::Value), "{input}");
                assert_eq!(error.syntax_error().unwrap().reason(), reason);
            }
            other => panic!("unexpected error for {input}: {other:?}"),
        }
    }
}

#[test]
fn token_first_preserves_escaped_quotes_and_classifies_incomplete_escapes() {
    let owner = JsonDecoder::unlimited()
        .decode_unique_validated_str(
            r#""before\"after""#,
            &options(JsonValidatedAdmissionPolicy::TokenFirst),
            |_, _| Ok::<_, Infallible>(JsonNumberPolicy::Preserve),
        )
        .expect("escaped quotes are part of the string token");
    assert_eq!(owner.as_value(), &serde_json::Value::String("before\"after".to_owned()));

    for input in ["\"tail\\", "\"\\u12"] {
        match decode_error(input, JsonValidatedAdmissionPolicy::TokenFirst) {
            JsonValidatedDecodeError::Decode { error, context } => {
                assert_eq!(context, Some(Context::String), "{input:?}");
                assert_eq!(
                    error.syntax_error().unwrap().reason(),
                    crate::decode::JsonSyntaxErrorReason::UnexpectedEnd,
                    "{input:?}"
                );
            }
            other => panic!("unexpected incomplete-string result for {input:?}: {other:?}"),
        }
    }
}

#[test]
fn token_first_uses_maximal_number_tokens_and_rejects_invalid_candidates() {
    for (input, expected_context, expected_reason) in [
        (
            "1x",
            Context::TrailingInput,
            crate::decode::JsonSyntaxErrorReason::TrailingCharacters,
        ),
        (
            "01",
            Context::Number,
            crate::decode::JsonSyntaxErrorReason::InvalidNumber,
        ),
        (
            "1-2",
            Context::Number,
            crate::decode::JsonSyntaxErrorReason::InvalidNumber,
        ),
        (
            "{\"a\":1-2}",
            Context::Number,
            crate::decode::JsonSyntaxErrorReason::InvalidNumber,
        ),
        (
            "1e+",
            Context::Number,
            crate::decode::JsonSyntaxErrorReason::InvalidNumber,
        ),
    ] {
        match decode_error(input, JsonValidatedAdmissionPolicy::TokenFirst) {
            JsonValidatedDecodeError::Decode { error, context } => {
                assert_eq!(context, Some(expected_context), "{input}");
                assert_eq!(error.syntax_error().unwrap().reason(), expected_reason, "{input}");
            }
            other => panic!("unexpected error for {input}: {other:?}"),
        }
    }
    match decode_error("[1x]", JsonValidatedAdmissionPolicy::TokenFirst) {
        JsonValidatedDecodeError::Decode { error, context } => {
            assert_eq!(context, Some(Context::ArraySeparator));
            assert_eq!(
                error.syntax_error().unwrap().reason(),
                crate::decode::JsonSyntaxErrorReason::ExpectedCommaOrArrayEnd
            );
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn token_first_applies_decoded_string_and_key_budgets_after_source_admission() {
    let options = JsonValidatedDecodeOptions::builder()
        .admission_policy(JsonValidatedAdmissionPolicy::TokenFirst)
        .max_encoded_string_bytes(6)
        .build();
    let string_limits = JsonDecodeLimits::<JsonResource, usize>::builder()
        .value_limits(
            JsonValueLimits::<JsonResource, usize>::builder()
                .string_bytes_limit(ResourceLimit::new(JsonResource::StringBytes, 0))
                .build(),
        )
        .build();
    let string_error = JsonDecoder::with_limits(string_limits)
        .decode_unique_validated_str(r#""\u0061""#, &options, |_, _| {
            Ok::<_, Infallible>(JsonNumberPolicy::Preserve)
        })
        .expect_err("decoded one-byte content exceeds a zero-byte string budget");
    match string_error {
        JsonValidatedDecodeError::Decode { error, context } => {
            assert_eq!(context, None);
            assert_eq!(error.kind(), JsonDecodeErrorKind::Budget);
            assert_eq!(error.budget_error().unwrap().resource(), &JsonResource::StringBytes);
        }
        other => panic!("unexpected decoded-string budget result: {other:?}"),
    }

    let key_limits = JsonDecodeLimits::<JsonResource, usize>::builder()
        .value_limits(
            JsonValueLimits::<JsonResource, usize>::builder()
                .structure_limits(
                    StructureLimits::<JsonResource, usize>::builder()
                        .key_bytes_limit(ResourceLimit::new(JsonResource::KeyBytes, 0))
                        .build(),
                )
                .build(),
        )
        .build();
    let key_error = JsonDecoder::with_limits(key_limits)
        .decode_unique_validated_str(r#"{"\u0061":0}"#, &options, |_, _| {
            Ok::<_, Infallible>(JsonNumberPolicy::Preserve)
        })
        .expect_err("decoded one-byte key exceeds a zero-byte key budget");
    match key_error {
        JsonValidatedDecodeError::Decode { error, context } => {
            assert_eq!(context, None);
            assert_eq!(error.kind(), JsonDecodeErrorKind::Budget);
            assert_eq!(error.budget_error().unwrap().resource(), &JsonResource::KeyBytes);
        }
        other => panic!("unexpected decoded-key budget result: {other:?}"),
    }
}

#[test]
fn token_first_checks_raw_string_budgets_before_decoding_contents() {
    let options = JsonValidatedDecodeOptions::builder()
        .admission_policy(JsonValidatedAdmissionPolicy::TokenFirst)
        .max_encoded_string_bytes(1)
        .build();
    for (input, expected) in [("\"ab", None), ("\"abc", Some(2)), ("\"ab\"", Some(2))] {
        let error = JsonDecoder::unlimited()
            .decode_unique_validated_str(input, &options, |_, _| Ok::<_, Infallible>(JsonNumberPolicy::Preserve))
            .expect_err("string exceeds its limit or is unterminated");
        match (error, expected) {
            (JsonValidatedDecodeError::Decode { error, context }, None) => {
                assert_eq!(context, Some(Context::String));
                assert_eq!(
                    error.syntax_error().unwrap().reason(),
                    crate::decode::JsonSyntaxErrorReason::UnexpectedEnd
                );
            }
            (
                JsonValidatedDecodeError::Limit {
                    limit,
                    maximum,
                    observed,
                    ..
                },
                Some(expected_observed),
            ) => {
                assert_eq!(limit, JsonValidatedLimit::EncodedStringBytes);
                assert_eq!(maximum, 1);
                assert_eq!(observed, expected_observed);
            }
            (other, _) => panic!("unexpected string-budget result: {other:?}"),
        }
    }

    for (malformed, expected_reason) in [
        ("\"\\q\"", crate::decode::JsonSyntaxErrorReason::InvalidEscape),
        (
            "\"\\u12xz\"",
            crate::decode::JsonSyntaxErrorReason::InvalidUnicodeEscape,
        ),
    ] {
        match decode_error(malformed, JsonValidatedAdmissionPolicy::TokenFirst) {
            JsonValidatedDecodeError::Decode { error, context } => {
                assert_eq!(context, Some(Context::String));
                assert_eq!(error.syntax_error().unwrap().reason(), expected_reason);
            }
            other => panic!("unexpected malformed-string result: {other:?}"),
        }
    }
}

#[test]
fn token_first_checks_limit_and_escape_order_with_small_strings() {
    let options = JsonValidatedDecodeOptions::builder()
        .admission_policy(JsonValidatedAdmissionPolicy::TokenFirst)
        .max_encoded_string_bytes(0)
        .build();
    let empty = JsonDecoder::unlimited()
        .decode_unique_validated_str("\"\"", &options, |_, _| Ok::<_, Infallible>(JsonNumberPolicy::Preserve))
        .expect("empty string fits the zero-byte token limit");
    assert_eq!(empty.as_value(), &serde_json::Value::String(String::new()));

    for input in ["\"x\"", "\"\\q\""] {
        let error = JsonDecoder::unlimited()
            .decode_unique_validated_str(input, &options, |_, _| Ok::<_, Infallible>(JsonNumberPolicy::Preserve))
            .expect_err("encoded token limit is checked before string decoding");
        assert!(matches!(
            error,
            JsonValidatedDecodeError::Limit {
                limit: JsonValidatedLimit::EncodedStringBytes,
                maximum: 0,
                ..
            }
        ));
    }

    let one_byte = JsonValidatedDecodeOptions::builder()
        .admission_policy(JsonValidatedAdmissionPolicy::TokenFirst)
        .max_encoded_string_bytes(1)
        .build();
    let equal_limit = JsonDecoder::unlimited()
        .decode_unique_validated_str("\"x\"", &one_byte, |_, _| {
            Ok::<_, Infallible>(JsonNumberPolicy::Preserve)
        })
        .expect("a token at the configured limit is accepted");
    assert_eq!(equal_limit.as_value(), &serde_json::Value::String("x".to_owned()));
}

#[test]
fn token_first_reports_duplicates_before_the_following_colon() {
    let error = decode_error("{\"a\":1,\"a\" x}", JsonValidatedAdmissionPolicy::TokenFirst);
    match error {
        JsonValidatedDecodeError::Decode { error, .. } => {
            assert_eq!(
                error.materialization_error_reason(),
                Some(JsonMaterializationErrorReason::DuplicateKey)
            );
        }
        other => panic!("unexpected duplicate-key result: {other:?}"),
    }
}

#[test]
fn token_first_checks_complete_document_before_root_kind() {
    let options = JsonValidatedDecodeOptions::builder()
        .admission_policy(JsonValidatedAdmissionPolicy::TokenFirst)
        .root_kind(JsonRootKind::Object)
        .build();
    let error = JsonDecoder::unlimited()
        .decode_unique_validated_str("true x", &options, |_, _| {
            Ok::<_, Infallible>(JsonNumberPolicy::Preserve)
        })
        .expect_err("trailing input is checked before the root kind");
    match error {
        JsonValidatedDecodeError::Decode { error, context } => {
            assert_eq!(error.kind(), JsonDecodeErrorKind::InvalidJson);
            assert_eq!(context, Some(Context::TrailingInput));
        }
        other => panic!("unexpected root-kind result: {other:?}"),
    }
}

#[test]
fn token_first_keeps_number_path_and_exact_integer_projection() {
    let options = JsonValidatedDecodeOptions::builder()
        .admission_policy(JsonValidatedAdmissionPolicy::TokenFirst)
        .build();
    let mut seen = Vec::new();
    let owner = JsonDecoder::unlimited()
        .decode_unique_validated_str(r#"{"id":42,"ratio":1.25}"#, &options, |path, token| {
            seen.push((path.to_vec(), token.raw().to_owned()));
            Ok::<_, Infallible>(
                if matches!(path.last(), Some(JsonPathSegment::Key(key)) if key == "id") {
                    JsonNumberPolicy::ExactSafeInteger
                } else {
                    JsonNumberPolicy::Preserve
                },
            )
        })
        .expect("both numeric tokens are valid");
    assert_eq!(seen[0].0, [JsonPathSegment::Key("id".to_owned())]);
    assert_eq!(seen[0].1, "42");
    assert_eq!(seen[1].0, [JsonPathSegment::Key("ratio".to_owned())]);
    assert_eq!(owner.as_value()["id"].as_i64(), Some(42));
    assert_eq!(owner.as_value()["ratio"].as_f64(), Some(1.25));

    let token = JsonNumberToken::from_validated("9007199254740992", 0);
    assert!(token.exact_safe_integer().is_err());
}

#[test]
fn token_first_preserves_depth_and_node_limit_precedence() {
    let limits = JsonDecodeLimits::<JsonResource, usize>::builder().max_depth(0).build();
    let mut decoder = JsonDecoder::with_limits(limits);
    let limited_nodes = JsonValidatedDecodeOptions::builder()
        .admission_policy(JsonValidatedAdmissionPolicy::TokenFirst)
        .max_value_and_key_nodes(1)
        .build();
    let error = decoder
        .decode_unique_validated_str("[[]]", &limited_nodes, |_, _| {
            Ok::<_, Infallible>(JsonNumberPolicy::Preserve)
        })
        .expect_err("depth is checked before the node count");
    match error {
        JsonValidatedDecodeError::Decode { error, .. } => {
            assert_eq!(error.budget_error().unwrap().resource(), &JsonResource::Depth);
        }
        other => panic!("unexpected limit precedence result: {other:?}"),
    }

    let options = JsonValidatedDecodeOptions::builder()
        .admission_policy(JsonValidatedAdmissionPolicy::TokenFirst)
        .max_value_and_key_nodes(1)
        .build();
    let error = decode_error_with_options("{a:1}", &options);
    assert!(matches!(
        error,
        JsonValidatedDecodeError::Limit {
            limit: JsonValidatedLimit::ValueAndKeyNodes,
            maximum: 1,
            ..
        }
    ));
}

fn decode_error_with_options(
    input: &str,
    options: &JsonValidatedDecodeOptions,
) -> JsonValidatedDecodeError<Infallible> {
    JsonDecoder::unlimited()
        .decode_unique_validated_str(input, options, |_, _| Ok::<_, Infallible>(JsonNumberPolicy::Preserve))
        .expect_err("input must be rejected by the configured precedence")
}
