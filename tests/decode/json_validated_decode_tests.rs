// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use std::convert::Infallible;
use std::io::Cursor;

use qubit_budget::json::JsonDecodeLimits;
use qubit_json::decode::JsonDecoder;
use qubit_json::decode::JsonNumberPolicy;
use qubit_json::decode::JsonValidatedDecodeError;
use qubit_json::decode::JsonValidatedDecodeOptions;

#[derive(Clone)]
struct OpaqueResource;

#[test]
fn test_validated_entries_accept_non_debug_resource() {
    let options = JsonValidatedDecodeOptions::builder().build();
    let mut decoder = JsonDecoder::with_limits(JsonDecodeLimits::<OpaqueResource, u64>::new());
    let text = decoder
        .decode_unique_validated_str("7", &options, |_, _| Ok::<_, Infallible>(JsonNumberPolicy::Preserve))
        .unwrap_or_else(|_| panic!("string entry must accept an opaque resource"));
    assert_eq!(text.as_value().as_u64(), Some(7));

    let bytes = decoder
        .decode_unique_validated_utf8(b"8", &options, |_, _| Ok::<_, Infallible>(JsonNumberPolicy::Preserve))
        .unwrap_or_else(|_| panic!("byte entry must accept an opaque resource"));
    assert_eq!(bytes.as_value().as_u64(), Some(8));

    let mut input = Cursor::new(b"9");
    let streamed = decoder
        .decode_unique_validated_input(&mut input, &options, |_, _| {
            Ok::<_, Infallible>(JsonNumberPolicy::Preserve)
        })
        .unwrap_or_else(|_| panic!("input entry must accept an opaque resource"));
    assert_eq!(streamed.as_value().as_u64(), Some(9));

    let rejected = decoder.decode_unique_validated_str("10", &options, |_, _| Err::<JsonNumberPolicy, _>("rejected"));
    assert!(matches!(
        rejected,
        Err(JsonValidatedDecodeError::Policy {
            source: "rejected",
            offset: 0
        })
    ));
}

#[test]
fn test_validated_options_keep_zero_and_defaults() {
    let default = qubit_json::decode::JsonValidatedDecodeOptions::builder().build();
    assert_eq!(default.root_kind(), None);
    assert_eq!(
        default.number_model(),
        qubit_json::decode::JsonDecodeNumberModel::Integer64
    );
    assert_eq!(default.max_value_and_key_nodes(), None);
    assert_eq!(default.max_encoded_string_bytes(), None);

    let zero = qubit_json::decode::JsonValidatedDecodeOptions::builder()
        .max_value_and_key_nodes(0)
        .max_encoded_string_bytes(0)
        .build();
    assert_eq!(zero.max_value_and_key_nodes(), Some(0));
    assert_eq!(zero.max_encoded_string_bytes(), Some(0));
}

mod migrated_validated {
    use qubit_budget::json::JsonDecodeLimits;
    use qubit_json::decode::JsonDecodeNumberModel;
    use qubit_json::decode::JsonDecoder;
    use qubit_json::decode::JsonNumberPolicy;
    use qubit_json::decode::JsonPathSegment;
    use qubit_json::decode::JsonValidatedDecodeError;
    use qubit_json::decode::JsonValidatedDecodeOptions;
    #[test]
    fn test_validated_policy_sees_source_order_and_typed_path() {
        let mut seen = Vec::new();
        let options = JsonValidatedDecodeOptions::builder().build();
        let value = JsonDecoder::unlimited()
            .decode_unique_validated_str(r#"{"a":[{"0":1}],"b":2}"#, &options, |path, token| {
                seen.push((path.to_vec(), token.raw().to_owned(), token.offset()));
                Ok::<_, ()>(JsonNumberPolicy::Preserve)
            })
            .unwrap();
        assert_eq!(seen.len(), 2);
        assert!(matches!(
            &seen[0].0[..],
            [JsonPathSegment::Key(a), JsonPathSegment::Index(0), JsonPathSegment::Key(zero)]
                if a == "a" && zero == "0"
        ));
        assert_eq!(seen[0].1, "1");
        assert_eq!(seen[0].2, 11);
        assert_eq!(seen[1].1, "2");
        assert_eq!(seen[1].2, 19);
        assert!(value.as_value().is_object());
    }

    #[test]
    fn test_validated_policy_error_precedes_integer_range_error() {
        let options = JsonValidatedDecodeOptions::builder()
            .number_model(JsonDecodeNumberModel::Integer64)
            .build();
        let error = JsonDecoder::unlimited()
            .decode_unique_validated_str("18446744073709551616", &options, |_, _| Err("policy"))
            .unwrap_err();
        assert!(matches!(error, JsonValidatedDecodeError::Policy { offset: 0, .. }));
    }

    #[test]
    fn test_validated_policy_error_exposes_error_source_when_available() {
        let options = JsonValidatedDecodeOptions::builder().build();
        let error = JsonDecoder::unlimited()
            .decode_unique_validated_str("1", &options, |_, _| {
                Err::<JsonNumberPolicy, _>(std::io::Error::other("policy source"))
            })
            .unwrap_err();
        assert!(std::error::Error::source(&error).is_some());
    }

    #[test]
    fn test_validated_exact_safe_integer_projects_number() {
        let options = JsonValidatedDecodeOptions::builder().build();
        let owner = JsonDecoder::unlimited()
            .decode_unique_validated_str("90071992547409910e-1", &options, |_, _| {
                Ok::<_, ()>(JsonNumberPolicy::ExactSafeInteger)
            })
            .unwrap();
        assert_eq!(owner.as_value().as_i64(), Some(9_007_199_254_740_991));
    }

    #[test]
    fn test_validated_limits_count_key_nodes_and_encoded_string_bytes() {
        let one_node = JsonValidatedDecodeOptions::builder().max_value_and_key_nodes(1).build();
        assert!(
            JsonDecoder::unlimited()
                .decode_unique_validated_str("{}", &one_node, |_, _| Ok::<_, ()>(JsonNumberPolicy::Preserve))
                .is_ok()
        );
        let too_few = JsonValidatedDecodeOptions::builder().max_value_and_key_nodes(2).build();
        let error = JsonDecoder::unlimited()
            .decode_unique_validated_str(r#"{"a":0}"#, &too_few, |_, _| Ok::<_, ()>(JsonNumberPolicy::Preserve))
            .unwrap_err();
        assert!(matches!(
            error,
            JsonValidatedDecodeError::Limit {
                limit: qubit_json::decode::JsonValidatedLimit::ValueAndKeyNodes,
                maximum: 2,
                observed: 3,
                ..
            }
        ));

        let encoded = JsonValidatedDecodeOptions::builder()
            .max_encoded_string_bytes(2)
            .build();
        let error = JsonDecoder::unlimited()
            .decode_unique_validated_str(r#""a\n""#, &encoded, |_, _| Ok::<_, ()>(JsonNumberPolicy::Preserve))
            .unwrap_err();
        assert!(matches!(
            error,
            JsonValidatedDecodeError::Limit {
                limit: qubit_json::decode::JsonValidatedLimit::EncodedStringBytes,
                maximum: 2,
                observed: 3,
                ..
            }
        ));
    }

    #[test]
    fn test_validated_duplicate_key_and_trailing_input_keep_context() {
        let options = JsonValidatedDecodeOptions::builder().build();
        let duplicate = JsonDecoder::unlimited()
            .decode_unique_validated_str(r#"{"a":0,"\u0061":1}"#, &options, |_, _| {
                Ok::<_, ()>(JsonNumberPolicy::Preserve)
            })
            .unwrap_err();
        assert!(matches!(
            duplicate,
            JsonValidatedDecodeError::Decode {
                context: Some(qubit_json::decode::JsonDecodeContext::ObjectKey),
                ..
            }
        ));
        let trailing = JsonValidatedDecodeOptions::builder()
            .root_kind(qubit_json::decode::JsonRootKind::Object)
            .build();
        let error = JsonDecoder::unlimited()
            .decode_unique_validated_str("1x", &trailing, |_, _| Ok::<_, ()>(JsonNumberPolicy::Preserve))
            .unwrap_err();
        assert!(matches!(
            error,
            JsonValidatedDecodeError::Decode {
                context: Some(qubit_json::decode::JsonDecodeContext::TrailingInput),
                ..
            }
        ));
    }

    #[test]
    fn test_validated_depth_and_input_byte_accounting() {
        let limits = JsonDecodeLimits::builder().max_depth(1_usize).build();
        let mut decoder = JsonDecoder::with_limits(limits);
        assert!(decoder.decode_value_str("[0]").is_err());

        let limits = JsonDecodeLimits::builder().max_input_bytes(10_usize).build();
        let mut decoder = JsonDecoder::with_limits(limits);
        let options = JsonValidatedDecodeOptions::builder().build();
        let error = decoder
            .decode_unique_validated_str("1", &options, |_, _| Err("stop"))
            .unwrap_err();
        assert!(matches!(error, JsonValidatedDecodeError::Policy { .. }));
        assert_eq!(decoder.session().input_budget().unwrap().used(), 1);
    }

    #[test]
    fn test_validated_byte_input_and_explicit_number_fallback() {
        let options = JsonValidatedDecodeOptions::builder().build();
        let mut input = std::io::Cursor::new(b"[1,2]".to_vec());
        let value = JsonDecoder::unlimited()
            .decode_unique_validated_input(&mut input, &options, |path, token| {
                assert_eq!(
                    token.raw(),
                    if path.last() == Some(&JsonPathSegment::Index(0)) {
                        "1"
                    } else {
                        "2"
                    }
                );
                Ok::<_, ()>(JsonNumberPolicy::Preserve)
            })
            .unwrap();
        assert!(value.as_value().is_array());

        let mut input = std::io::Cursor::new(b"18446744073709551616".to_vec());
        let value: serde_json::Value = JsonDecoder::unlimited()
            .decode_input_with_number_model(&mut input, JsonDecodeNumberModel::Integer64WithFiniteFallback)
            .unwrap();
        let expected: serde_json::Value = serde_json::from_str("18446744073709551616").unwrap();
        assert_eq!(value.to_string(), expected.to_string());
        assert_eq!(value.is_f64(), expected.is_f64());
    }

    #[test]
    fn test_validated_deep_tree_has_no_implicit_127_limit() {
        let text = format!("{}0{}", "[".repeat(129), "]".repeat(129));
        let value = JsonDecoder::unlimited().decode_value_str(&text).unwrap();
        assert!(value.as_value().is_array());
    }

    #[test]
    fn test_validated_grammar_failures_keep_context() {
        let options = JsonValidatedDecodeOptions::builder().build();
        for (text, expected) in [
            ("[1,]", qubit_json::decode::JsonDecodeContext::Value),
            (r#"{"a" 0}"#, qubit_json::decode::JsonDecodeContext::Colon),
            (r#"{"a":01}"#, qubit_json::decode::JsonDecodeContext::Number),
            (r#"{"a":1x}"#, qubit_json::decode::JsonDecodeContext::ObjectSeparator),
            ("1x", qubit_json::decode::JsonDecodeContext::TrailingInput),
        ] {
            let error = JsonDecoder::unlimited()
                .decode_unique_validated_str(text, &options, |_, _| Ok::<_, ()>(JsonNumberPolicy::Preserve))
                .unwrap_err();
            assert!(
                matches!(error, JsonValidatedDecodeError::Decode { context: Some(context), .. } if context == expected),
                "{text}: {error}"
            );
        }
    }

    #[test]
    fn test_node_limit_zero_and_one_count_value_entries_and_keys() {
        let zero = JsonValidatedDecodeOptions::builder().max_value_and_key_nodes(0).build();
        assert!(matches!(
            JsonDecoder::unlimited()
                .decode_unique_validated_str("null", &zero, |_, _| Ok::<_, ()>(JsonNumberPolicy::Preserve)),
            Err(JsonValidatedDecodeError::Limit {
                maximum: 0,
                observed: 1,
                ..
            })
        ));
        let one = JsonValidatedDecodeOptions::builder().max_value_and_key_nodes(1).build();
        assert!(
            JsonDecoder::unlimited()
                .decode_unique_validated_str("{}", &one, |_, _| Ok::<_, ()>(JsonNumberPolicy::Preserve))
                .is_ok()
        );
        assert!(matches!(
            JsonDecoder::unlimited()
                .decode_unique_validated_str(r#"{"a":0}"#, &one, |_, _| Ok::<_, ()>(JsonNumberPolicy::Preserve)),
            Err(JsonValidatedDecodeError::Limit {
                maximum: 1,
                observed: 2,
                ..
            })
        ));
    }

    #[test]
    fn test_encoded_string_limits_use_source_bytes_and_decoded_budget_is_separate() {
        let encoded = JsonValidatedDecodeOptions::builder()
            .max_encoded_string_bytes(3)
            .build();
        assert!(
            JsonDecoder::unlimited()
                .decode_unique_validated_str(r#""\u0061""#, &encoded, |_, _| Ok::<_, ()>(JsonNumberPolicy::Preserve))
                .is_err()
        );
        assert!(
            JsonDecoder::unlimited()
                .decode_unique_validated_str(r#""😀""#, &encoded, |_, _| Ok::<_, ()>(JsonNumberPolicy::Preserve))
                .is_err()
        );
        let limits = JsonDecodeLimits::builder().max_string_bytes(3_usize).build();
        assert!(JsonDecoder::with_limits(limits).decode_value_str(r#""😀""#).is_err());
    }

    #[test]
    fn test_encoded_string_limits_accept_exact_boundary_for_escape_and_emoji() {
        let escape = JsonValidatedDecodeOptions::builder()
            .max_encoded_string_bytes(6)
            .build();
        assert!(
            JsonDecoder::unlimited()
                .decode_unique_validated_str(r#""\u0061""#, &escape, |_, _| Ok::<_, ()>(JsonNumberPolicy::Preserve))
                .is_ok()
        );
        let below_escape = JsonValidatedDecodeOptions::builder()
            .max_encoded_string_bytes(5)
            .build();
        assert!(matches!(
            JsonDecoder::unlimited().decode_unique_validated_str(r#""\u0061""#, &below_escape, |_, _| Ok::<_, ()>(
                JsonNumberPolicy::Preserve
            )),
            Err(JsonValidatedDecodeError::Limit {
                maximum: 5,
                observed: 6,
                ..
            })
        ));

        let emoji = JsonValidatedDecodeOptions::builder()
            .max_encoded_string_bytes(4)
            .build();
        assert!(
            JsonDecoder::unlimited()
                .decode_unique_validated_str(r#""😀""#, &emoji, |_, _| Ok::<_, ()>(JsonNumberPolicy::Preserve))
                .is_ok()
        );
        let below_emoji = JsonValidatedDecodeOptions::builder()
            .max_encoded_string_bytes(3)
            .build();
        let below_emoji_result =
            JsonDecoder::unlimited().decode_unique_validated_str(r#""😀""#, &below_emoji, |_, _| {
                Ok::<_, ()>(JsonNumberPolicy::Preserve)
            });
        assert!(matches!(
            below_emoji_result,
            Err(JsonValidatedDecodeError::Limit {
                maximum: 3,
                observed: 4,
                ..
            })
        ));
        let zero = JsonValidatedDecodeOptions::builder()
            .max_encoded_string_bytes(0)
            .build();
        let unterminated = JsonDecoder::unlimited()
            .decode_unique_validated_str(r#""abc"#, &zero, |_, _| Ok::<_, ()>(JsonNumberPolicy::Preserve))
            .unwrap_err();
        assert!(matches!(unterminated, JsonValidatedDecodeError::Decode { .. }));
    }

    #[test]
    fn test_policy_error_and_panic_stop_without_committing_values() {
        let limits = JsonDecodeLimits::builder()
            .max_input_bytes(64_usize)
            .max_nodes(8_usize)
            .build();
        let mut decoder = JsonDecoder::with_limits(limits);
        let options = JsonValidatedDecodeOptions::builder().build();
        let mut calls = 0;
        let error = decoder
            .decode_unique_validated_str("[1,2]", &options, |_, _| {
                calls += 1;
                Err("secret-token")
            })
            .unwrap_err();
        assert_eq!(calls, 1);
        assert!(matches!(error, JsonValidatedDecodeError::Policy { offset: 1, .. }));
        assert!(!format!("{error}").contains("secret-token"));
        assert!(!format!("{error:?}").contains("secret-token"));
        assert_eq!(decoder.session().input_budget().unwrap().used(), 5);
        assert_eq!(decoder.session().value_budget().used_nodes(), Some(0));

        let panic_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = decoder.decode_unique_validated_str("[1,2]", &options, |_, _| -> Result<JsonNumberPolicy, ()> {
                panic!("policy panic")
            });
        }));
        assert!(panic_result.is_err());
        assert_eq!(decoder.session().input_budget().unwrap().used(), 10);
        assert_eq!(decoder.session().value_budget().used_nodes(), Some(0));
        assert!(decoder.decode_value_str("true").is_ok());
    }

    #[test]
    fn test_policy_side_effect_precedes_later_syntax_failure() {
        let options = JsonValidatedDecodeOptions::builder().build();
        let mut seen = Vec::new();
        let error = JsonDecoder::unlimited()
            .decode_unique_validated_str("[1,]", &options, |path, token| {
                seen.push((path.to_vec(), token.raw().to_owned()));
                Ok::<_, ()>(JsonNumberPolicy::Preserve)
            })
            .unwrap_err();
        assert_eq!(seen.len(), 1);
        assert_eq!(seen[0].1, "1");
        assert!(matches!(
            error,
            JsonValidatedDecodeError::Decode {
                context: Some(qubit_json::decode::JsonDecodeContext::Value),
                ..
            }
        ));
    }

    #[test]
    fn test_root_kind_is_checked_after_complete_validated_scan() {
        let object_only = JsonValidatedDecodeOptions::builder()
            .root_kind(qubit_json::decode::JsonRootKind::Object)
            .build();
        let error = JsonDecoder::unlimited()
            .decode_unique_validated_str("[]", &object_only, |_, _| {
                Ok::<_, std::io::Error>(JsonNumberPolicy::Preserve)
            })
            .unwrap_err();
        assert!(matches!(
            &error,
            JsonValidatedDecodeError::Decode {
                context: None,
                error,
            } if error.kind() == qubit_json::decode::JsonDecodeErrorKind::UnexpectedTopLevel
        ));
        assert!(error.to_string().contains("expected object, got array"));
        let JsonValidatedDecodeError::Decode { error, .. } = &error else {
            panic!("root mismatch must retain the underlying decode error");
        };
        assert_eq!(
            error.expected_top_level(),
            Some(qubit_json::decode::JsonRootKind::Object)
        );
        assert_eq!(error.actual_top_level(), Some(qubit_json::decode::JsonRootKind::Array));
        assert!(std::error::Error::source(&error).is_none());

        let malformed = JsonDecoder::unlimited()
            .decode_unique_validated_str("[]x", &object_only, |_, _| {
                Ok::<_, std::io::Error>(JsonNumberPolicy::Preserve)
            })
            .unwrap_err();
        assert!(matches!(
            &malformed,
            JsonValidatedDecodeError::Decode {
                context: Some(qubit_json::decode::JsonDecodeContext::TrailingInput),
                error,
            } if error.kind() == qubit_json::decode::JsonDecodeErrorKind::InvalidJson
        ));
    }

    #[test]
    fn test_number_model_and_exact_policy_error_boundaries() {
        let mut strict_input = std::io::Cursor::new(b"18446744073709551616".to_vec());
        let strict_error: qubit_json::decode::JsonDecodeError = JsonDecoder::unlimited()
            .decode_input_with_number_model::<serde_json::Value, _>(&mut strict_input, JsonDecodeNumberModel::Integer64)
            .unwrap_err();
        assert_eq!(
            strict_error.kind(),
            qubit_json::decode::JsonDecodeErrorKind::InvalidJson
        );

        let mut out_of_float_range = std::io::Cursor::new(b"1e400".to_vec());
        assert!(
            JsonDecoder::unlimited()
                .decode_input_with_number_model::<serde_json::Value, _>(
                    &mut out_of_float_range,
                    JsonDecodeNumberModel::Integer64WithFiniteFallback,
                )
                .is_err()
        );

        let fallback_options = JsonValidatedDecodeOptions::builder()
            .number_model(JsonDecodeNumberModel::Integer64WithFiniteFallback)
            .build();
        let fallback = JsonDecoder::unlimited()
            .decode_unique_validated_str("18446744073709551616", &fallback_options, |_, _| {
                Ok::<_, ()>(JsonNumberPolicy::Preserve)
            })
            .unwrap();
        let number = fallback.as_value().as_number().unwrap();
        let expected = serde_json::Value::Number(serde_json::Number::from_f64(18446744073709551616_f64).unwrap());
        assert!(number.is_f64());
        assert_eq!(number.to_string(), expected.as_number().unwrap().to_string());

        let options = JsonValidatedDecodeOptions::builder().build();
        for (token, expected) in [
            ("1.5", qubit_json::decode::JsonNumberTokenError::Fractional),
            (
                "9007199254740992",
                qubit_json::decode::JsonNumberTokenError::OutOfSafeIntegerRange,
            ),
        ] {
            let error = JsonDecoder::unlimited()
                .decode_unique_validated_str(token, &options, |_, _| {
                    Ok::<_, std::io::Error>(JsonNumberPolicy::ExactSafeInteger)
                })
                .unwrap_err();
            assert!(matches!(
                &error,
                JsonValidatedDecodeError::ExactSafeInteger { source, offset: 0 }
                    if *source == expected
            ));
            assert!(error.to_string().contains("exact safe integer"));
            assert!(std::error::Error::source(&error).is_some());
        }

        let exact = JsonValidatedDecodeOptions::builder()
            .number_model(JsonDecodeNumberModel::Integer64WithFiniteFallback)
            .build();
        let error = JsonDecoder::unlimited()
            .decode_unique_validated_str("1e999", &exact, |_, _| Ok::<_, ()>(JsonNumberPolicy::ExactSafeInteger))
            .unwrap_err();
        assert!(matches!(
            error,
            JsonValidatedDecodeError::Decode {
                context: Some(qubit_json::decode::JsonDecodeContext::Number),
                error,
            } if error.kind() == qubit_json::decode::JsonDecodeErrorKind::InvalidJson
        ));
    }
}

mod api_gaps_integration {
    use std::io::Cursor;

    use qubit_budget::json::JsonDecodeLimits;
    use qubit_json::decode::DiagnosticPolicy;
    use qubit_json::decode::JsonDecodeContext;
    use qubit_json::decode::JsonDecodeErrorKind;
    use qubit_json::decode::JsonDecodeNumberModel;
    use qubit_json::decode::JsonDecoder;
    use qubit_json::decode::JsonNumberPolicy;
    use qubit_json::decode::JsonValidatedAdmissionPolicy;
    use qubit_json::decode::JsonValidatedDecodeError;
    use qubit_json::decode::JsonValidatedDecodeOptions;
    use qubit_json::decode::JsonValidatedLimit;

    fn token_first() -> JsonValidatedDecodeOptions {
        JsonValidatedDecodeOptions::builder()
            .admission_policy(JsonValidatedAdmissionPolicy::TokenFirst)
            .build()
    }

    fn preserve() -> impl for<'call> FnMut(
        &'call [qubit_json::decode::JsonPathSegment],
        qubit_json::decode::JsonNumberToken<'call>,
    ) -> Result<JsonNumberPolicy, ()> {
        |_, _| Ok(JsonNumberPolicy::Preserve)
    }

    fn assert_context(error: JsonValidatedDecodeError<()>, expected: JsonDecodeContext) {
        let JsonValidatedDecodeError::Decode { error, context } = error else {
            panic!("expected syntax failure");
        };
        assert_eq!(context, Some(expected));
        assert_eq!(error.syntax_error().and_then(|syntax| syntax.context()), context);
    }

    #[test]
    fn test_token_first_is_shared_by_str_utf8_and_input() {
        let text = r#"{"a":truex}"#;
        let options = token_first();
        let str_error = JsonDecoder::unlimited()
            .decode_unique_validated_str(text, &options, preserve())
            .unwrap_err();
        assert_context(str_error, JsonDecodeContext::ObjectSeparator);
        let utf8_error = JsonDecoder::unlimited()
            .decode_unique_validated_utf8(text.as_bytes(), &options, preserve())
            .unwrap_err();
        assert_context(utf8_error, JsonDecodeContext::ObjectSeparator);
        let mut input = Cursor::new(text.as_bytes());
        let input_error = JsonDecoder::unlimited()
            .decode_unique_validated_input(&mut input, &options, preserve())
            .unwrap_err();
        assert_context(input_error, JsonDecodeContext::ObjectSeparator);
    }

    #[test]
    fn test_failure_keeps_input_charge_but_next_document_can_commit() {
        let limits = JsonDecodeLimits::builder()
            .max_input_bytes(16_usize)
            .max_nodes(4_usize)
            .build();
        let mut decoder = JsonDecoder::with_limits(limits);
        let error = decoder
            .decode_unique_validated_str("[truex]", &token_first(), preserve())
            .unwrap_err();
        assert!(matches!(error, JsonValidatedDecodeError::Decode { .. }));
        assert_eq!(decoder.session().input_budget().unwrap().used(), 7);
        assert_eq!(decoder.session().value_budget().used_nodes(), Some(0));
        let value = decoder
            .decode_unique_validated_str("[1]", &token_first(), preserve())
            .unwrap();
        assert_eq!(value.as_value()[0].as_i64(), Some(1));
        assert_eq!(decoder.session().input_budget().unwrap().used(), 10);
        assert_eq!(decoder.session().value_budget().used_nodes(), Some(2));
    }

    #[test]
    fn test_input_limit_precedes_invalid_utf8() {
        let limits = JsonDecodeLimits::builder().max_input_bytes(0_usize).build();
        let error = JsonDecoder::with_limits(limits)
            .decode_unique_validated_utf8(&[0xff], &token_first(), preserve())
            .unwrap_err();
        let JsonValidatedDecodeError::Decode { error, context } = error else {
            panic!("expected input budget failure");
        };
        assert_eq!(error.kind(), JsonDecodeErrorKind::Budget);
        assert_eq!(context, None);
    }

    #[test]
    fn test_depth_precedes_nodes_and_key_nodes_precede_key_syntax() {
        let nodes = JsonValidatedDecodeOptions::builder().max_value_and_key_nodes(1).build();
        let limits = JsonDecodeLimits::builder().max_depth(1_usize).build();
        let error = JsonDecoder::with_limits(limits)
            .decode_unique_validated_str("[0]", &nodes, preserve())
            .unwrap_err();
        let JsonValidatedDecodeError::Decode { error, .. } = error else {
            panic!("depth budget failure must be a decode error");
        };
        let qubit_json::decode::JsonDecodeErrorSource::Budget { source, .. } = error.into_source() else {
            panic!("depth limit must retain its measured budget source");
        };
        assert_eq!(source.resource(), &qubit_budget::json::JsonResource::Depth);
        let error = JsonDecoder::unlimited()
            .decode_unique_validated_str("{x:0}", &nodes, preserve())
            .unwrap_err();
        assert!(matches!(
            error,
            JsonValidatedDecodeError::Limit {
                limit: JsonValidatedLimit::ValueAndKeyNodes,
                maximum: 1,
                observed: 2,
                offset: 1,
            }
        ));
    }

    #[test]
    fn test_c01_three_entries_match_values_paths_offsets_and_charge_once() {
        let text = r#"{"a":[1,2]}"#;
        let options = token_first();
        let mut str_calls = Vec::new();
        let mut a = JsonDecoder::with_limits(JsonDecodeLimits::builder().max_input_bytes(64_usize).build());
        let str_value = a
            .decode_unique_validated_str(text, &options, |path, token| {
                str_calls.push((path.to_vec(), token.offset()));
                Ok::<_, ()>(JsonNumberPolicy::Preserve)
            })
            .unwrap();
        let mut utf8_calls = Vec::new();
        let mut b = JsonDecoder::with_limits(JsonDecodeLimits::builder().max_input_bytes(64_usize).build());
        let utf8_value = b
            .decode_unique_validated_utf8(text.as_bytes(), &options, |path, token| {
                utf8_calls.push((path.to_vec(), token.offset()));
                Ok::<_, ()>(JsonNumberPolicy::Preserve)
            })
            .unwrap();
        let mut input_calls = Vec::new();
        let mut c = JsonDecoder::with_limits(JsonDecodeLimits::builder().max_input_bytes(64_usize).build());
        let input_value = c
            .decode_unique_validated_input(&mut Cursor::new(text.as_bytes()), &options, |path, token| {
                input_calls.push((path.to_vec(), token.offset()));
                Ok::<_, ()>(JsonNumberPolicy::Preserve)
            })
            .unwrap();
        assert_eq!(str_value, utf8_value);
        assert_eq!(utf8_value, input_value);
        assert_eq!(str_calls, utf8_calls);
        assert_eq!(utf8_calls, input_calls);
        assert_eq!(str_calls.len(), 2);
        assert_eq!(a.session().input_budget().unwrap().used(), text.len());
        assert_eq!(b.session().input_budget().unwrap().used(), text.len());
        assert_eq!(c.session().input_budget().unwrap().used(), text.len());
    }

    #[test]
    fn test_c02_duplicate_key_materialization_precedes_missing_colon() {
        let error = JsonDecoder::unlimited()
            .decode_unique_validated_str(r#"{"a":0,"\u0061" 1}"#, &token_first(), preserve())
            .unwrap_err();
        assert!(matches!(error, JsonValidatedDecodeError::Decode {
            context: Some(JsonDecodeContext::ObjectKey), error,
        } if error.materialization_error_reason() == Some(qubit_json::decode::JsonMaterializationErrorReason::DuplicateKey)));
    }

    #[test]
    fn test_c03_encoded_limit_precedes_decoded_string_budget() {
        let options = JsonValidatedDecodeOptions::builder()
            .max_encoded_string_bytes(2)
            .build();
        let limits = JsonDecodeLimits::builder().max_string_bytes(0_usize).build();
        let error = JsonDecoder::with_limits(limits)
            .decode_unique_validated_str(r#""\u0061""#, &options, preserve())
            .unwrap_err();
        assert!(matches!(
            error,
            JsonValidatedDecodeError::Limit {
                limit: JsonValidatedLimit::EncodedStringBytes,
                maximum: 2,
                observed: 6,
                ..
            }
        ));
    }

    #[test]
    fn test_c04_decoded_string_budget_remains_budget_error() {
        let limits = JsonDecodeLimits::builder().max_string_bytes(0_usize).build();
        let error = JsonDecoder::with_limits(limits)
            .decode_unique_validated_str(r#""a""#, &token_first(), preserve())
            .unwrap_err();
        assert!(
            matches!(error, JsonValidatedDecodeError::Decode { error, context: None }
            if error.kind() == JsonDecodeErrorKind::Budget)
        );
    }

    #[test]
    fn test_c05_object_key_decoded_budget_is_checked_after_encoded_budget() {
        let options = JsonValidatedDecodeOptions::builder()
            .max_encoded_string_bytes(0)
            .build();
        let limits = JsonDecodeLimits::builder().max_key_bytes(0_usize).build();
        let error = JsonDecoder::with_limits(limits)
            .decode_unique_validated_str(r#"{"a":0}"#, &options, preserve())
            .unwrap_err();
        assert!(matches!(
            error,
            JsonValidatedDecodeError::Limit {
                limit: JsonValidatedLimit::EncodedStringBytes,
                maximum: 0,
                observed: 1,
                offset: 1,
            }
        ));
    }

    #[test]
    fn test_c06_policy_error_precedes_range_error_and_hides_sentinel() {
        let mut calls = 0;
        let error = JsonDecoder::unlimited()
            .decode_unique_validated_str("1e400", &token_first(), |_, _| {
                calls += 1;
                Err::<JsonNumberPolicy, _>("private-sentinel")
            })
            .unwrap_err();
        assert_eq!(calls, 1);
        assert!(matches!(error, JsonValidatedDecodeError::Policy { offset: 0, .. }));
        assert!(!error.to_string().contains("private-sentinel"));
        assert!(!format!("{error:?}").contains("private-sentinel"));
    }

    #[test]
    fn test_c07_exact_policy_observes_float_range_failure_first() {
        let options = JsonValidatedDecodeOptions::builder()
            .number_model(JsonDecodeNumberModel::Integer64WithFiniteFallback)
            .build();
        let mut calls = 0;
        let error = JsonDecoder::unlimited()
            .decode_unique_validated_str("1e400", &options, |_, _| {
                calls += 1;
                Ok::<_, ()>(JsonNumberPolicy::ExactSafeInteger)
            })
            .unwrap_err();
        assert_eq!(calls, 1);
        let JsonValidatedDecodeError::Decode { error, context } = error else {
            panic!("float range failure must remain a decode error");
        };
        assert_eq!(context, Some(JsonDecodeContext::Number));
        let syntax = error
            .syntax_error()
            .expect("invalid JSON decode must retain its syntax source");
        assert_eq!(
            syntax.reason(),
            qubit_json::decode::JsonSyntaxErrorReason::FloatOutOfRange
        );
    }

    #[test]
    fn test_c08_exact_policy_projects_fallback_decimal_without_f64_rounding() {
        let options = JsonValidatedDecodeOptions::builder()
            .number_model(JsonDecodeNumberModel::Integer64WithFiniteFallback)
            .build();
        let value = JsonDecoder::unlimited()
            .decode_unique_validated_str("90071992547409910e-1", &options, |_, _| {
                Ok::<_, ()>(JsonNumberPolicy::ExactSafeInteger)
            })
            .unwrap();
        assert_eq!(value.as_value().as_i64(), Some(9_007_199_254_740_991));
    }

    #[test]
    fn test_c09_exact_policy_rejects_unsafe_integer_and_fraction() {
        for (text, expected) in [
            (
                "9007199254740992",
                qubit_json::decode::JsonNumberTokenError::OutOfSafeIntegerRange,
            ),
            ("0.1", qubit_json::decode::JsonNumberTokenError::Fractional),
        ] {
            let error = JsonDecoder::unlimited()
                .decode_unique_validated_str(text, &token_first(), |_, _| {
                    Ok::<_, ()>(JsonNumberPolicy::ExactSafeInteger)
                })
                .unwrap_err();
            assert!(
                matches!(error, JsonValidatedDecodeError::ExactSafeInteger { source, offset: 0 }
                if source == expected)
            );
        }
    }

    #[test]
    fn test_c10_trailing_input_precedes_root_kind_check() {
        let object = JsonValidatedDecodeOptions::builder()
            .root_kind(qubit_json::decode::JsonRootKind::Object)
            .build();
        let trailing = JsonDecoder::unlimited()
            .decode_unique_validated_str("[]x", &object, preserve())
            .unwrap_err();
        assert!(matches!(trailing, JsonValidatedDecodeError::Decode {
            context: Some(JsonDecodeContext::TrailingInput), error,
        } if error.kind() == JsonDecodeErrorKind::InvalidJson));
        let root = JsonDecoder::unlimited()
            .decode_unique_validated_str("[]", &object, preserve())
            .unwrap_err();
        assert!(matches!(root, JsonValidatedDecodeError::Decode { error, context: None }
            if error.kind() == JsonDecodeErrorKind::UnexpectedTopLevel));
    }

    #[test]
    fn test_c11_failed_values_are_rolled_back_and_next_value_commits() {
        let limits = JsonDecodeLimits::builder()
            .max_input_bytes(16_usize)
            .max_nodes(4_usize)
            .build();
        let mut decoder = JsonDecoder::with_limits(limits);
        assert!(
            decoder
                .decode_unique_validated_str("[truex]", &token_first(), preserve())
                .is_err()
        );
        assert_eq!(decoder.session().input_budget().unwrap().used(), 7);
        assert_eq!(decoder.session().value_budget().used_nodes(), Some(0));
        decoder
            .decode_unique_validated_str("[1]", &token_first(), preserve())
            .unwrap();
        assert_eq!(decoder.session().input_budget().unwrap().used(), 10);
        assert_eq!(decoder.session().value_budget().used_nodes(), Some(2));
    }

    #[test]
    fn test_c12_policy_panic_rolls_back_values_and_decoder_remains_usable() {
        let limits = JsonDecodeLimits::builder()
            .max_input_bytes(32_usize)
            .max_nodes(8_usize)
            .build();
        let mut decoder = JsonDecoder::with_limits(limits);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ =
                decoder.decode_unique_validated_str("[1]", &token_first(), |_, _| -> Result<JsonNumberPolicy, ()> {
                    panic!("policy panic")
                });
        }));
        assert!(result.is_err());
        assert_eq!(decoder.session().input_budget().unwrap().used(), 3);
        assert_eq!(decoder.session().value_budget().used_nodes(), Some(0));
        assert_eq!(
            decoder.decode_value_str("true").unwrap().as_value(),
            &serde_json::Value::Bool(true)
        );
    }

    #[test]
    fn test_c13_token_first_does_not_stick_to_default_owner_or_typed_decode() {
        let mut decoder = JsonDecoder::unlimited();
        assert_context(
            decoder
                .decode_unique_validated_str("truex", &token_first(), preserve())
                .unwrap_err(),
            JsonDecodeContext::TrailingInput,
        );
        let owner_error = decoder.decode_value_str("truex").unwrap_err();
        assert_eq!(owner_error.kind(), JsonDecodeErrorKind::InvalidJson);
        assert_eq!(
            owner_error.syntax_error().and_then(|syntax| syntax.context()),
            Some(JsonDecodeContext::Value)
        );
        assert_eq!(
            owner_error.syntax_error().map(|syntax| syntax.reason()),
            Some(qubit_json::decode::JsonSyntaxErrorReason::UnexpectedByte)
        );
        let typed_error = decoder.decode_str::<serde_json::Value>("truex").unwrap_err();
        assert_eq!(
            typed_error.syntax_error().and_then(|syntax| syntax.context()),
            Some(JsonDecodeContext::Value)
        );
        assert_eq!(
            typed_error.syntax_error().map(|syntax| syntax.reason()),
            Some(qubit_json::decode::JsonSyntaxErrorReason::UnexpectedByte)
        );
    }

    #[test]
    fn test_c14_pretty_formatter_remains_syntax_only_and_accepts_duplicates_and_wide_numbers() {
        let pretty = qubit_json::format::JsonPrettyOptions::builder().build().unwrap();
        for text in ["1e400", r#"{"a":0,"a":1}"#] {
            assert!(qubit_json::format::format_json_tokens(text.as_bytes(), &pretty, None).is_ok());
        }
    }

    #[cfg(feature = "jcs")]
    #[test]
    fn test_c15_jcs_keeps_binary64_number_and_default_separator_behavior() {
        let mut encoder = qubit_json::encode::JcsEncoder::unlimited();
        let number = serde_json::Value::from(1.0_f64);
        assert_eq!(encoder.to_vec(&number).unwrap(), b"1");
        let pretty = qubit_json::format::JsonPrettyOptions::builder().build().unwrap();
        let error = qubit_json::format::format_json_tokens(b"truex", &pretty, None).unwrap_err();
        assert_eq!(error.kind(), qubit_json::format::JsonPrettyErrorKind::Syntax);
    }

    #[test]
    fn test_c16_raw_value_syntax_error_keeps_context_and_encode_category() {
        use serde::Serialize;
        use serde::Serializer;
        use serde::ser::SerializeStruct;
        struct BadRaw;
        impl Serialize for BadRaw {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                let mut state =
                    serializer.serialize_struct(concat!("$", "serde_json", ":", ":private::RawValue"), 1)?;
                state.serialize_field(concat!("$", "serde_json", ":", ":private::RawValue"), "[1")?;
                state.end()
            }
        }
        let error = qubit_json::encode::JsonEncoder::unlimited()
            .to_vec(&BadRaw)
            .unwrap_err();
        assert_eq!(error.kind(), qubit_json::encode::JsonEncodeErrorKind::InvalidRawJson);
        assert_eq!(
            error.syntax_error().and_then(|syntax| syntax.context()),
            Some(JsonDecodeContext::ArraySeparator)
        );
        assert!(error.budget_error().is_none());
    }

    #[test]
    fn test_c17_valid_successes_match_between_policies_and_number_models() {
        let text = r#"{"k\u0065y":[-0,90071992547409910e-1,{"n":9223372036854775807}]}"#;
        for model in [
            JsonDecodeNumberModel::Integer64,
            JsonDecodeNumberModel::Integer64WithFiniteFallback,
        ] {
            let immediate = JsonValidatedDecodeOptions::builder().number_model(model).build();
            let token_first = JsonValidatedDecodeOptions::builder()
                .number_model(model)
                .admission_policy(JsonValidatedAdmissionPolicy::TokenFirst)
                .build();
            let a = JsonDecoder::unlimited()
                .decode_unique_validated_str(text, &immediate, preserve())
                .unwrap();
            let b = JsonDecoder::unlimited()
                .decode_unique_validated_str(text, &token_first, preserve())
                .unwrap();
            assert_eq!(a, b);
        }
        let fallback = JsonValidatedDecodeOptions::builder()
            .number_model(JsonDecodeNumberModel::Integer64WithFiniteFallback)
            .build();
        let token_first_fallback = JsonValidatedDecodeOptions::builder()
            .number_model(JsonDecodeNumberModel::Integer64WithFiniteFallback)
            .admission_policy(JsonValidatedAdmissionPolicy::TokenFirst)
            .build();
        let a = JsonDecoder::unlimited()
            .decode_unique_validated_str("18446744073709551616", &fallback, preserve())
            .unwrap();
        let b = JsonDecoder::unlimited()
            .decode_unique_validated_str("18446744073709551616", &token_first_fallback, preserve())
            .unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn test_c18_redacted_and_detailed_sources_keep_syntax_context_and_position() {
        for policy in [DiagnosticPolicy::Redacted, DiagnosticPolicy::Detailed] {
            let options = JsonValidatedDecodeOptions::builder().build();
            let error = JsonDecoder::unlimited()
                .with_diagnostic_policy(policy)
                .decode_unique_validated_str("[1,]", &options, preserve())
                .unwrap_err();
            let JsonValidatedDecodeError::Decode { error, context } = error else {
                panic!("expected syntax error");
            };
            assert_eq!(context, Some(JsonDecodeContext::Value));
            let source = error.into_source();
            let qubit_json::decode::JsonDecodeErrorSource::InvalidJson { syntax, .. } = source else {
                panic!("expected invalid JSON source");
            };
            assert_eq!(syntax.context(), Some(JsonDecodeContext::Value));
            assert_eq!(syntax.offset(), 3);
        }
    }
}
