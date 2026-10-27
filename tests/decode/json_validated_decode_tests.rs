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
        let number = value.as_number().unwrap();
        assert!(number.is_f64() || number.to_string() == "18446744073709551616");
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
        assert!(number.as_f64().is_some() || number.to_string() == "18446744073709551616");

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
