// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Public behavior tests for structured JSON syntax errors.

mod error_tests;

mod api_gaps_context {
    use qubit_json::decode::DiagnosticPolicy;
    use qubit_json::decode::JsonDecodeContext;
    use qubit_json::decode::JsonDecoder;
    use qubit_json::decode::JsonNumberPolicy;
    use qubit_json::decode::JsonSyntaxError;
    use qubit_json::decode::JsonSyntaxErrorReason;
    use qubit_json::decode::JsonValidatedDecodeError;
    use qubit_json::decode::JsonValidatedDecodeOptions;

    #[test]
    fn test_manual_context_keeps_location_reason_and_display() {
        let old = JsonSyntaxError::new(7, 2, 3, JsonSyntaxErrorReason::UnexpectedEnd);
        assert_eq!(old.context(), None);
        let new = old.with_context(JsonDecodeContext::String);
        assert_eq!(new.context(), Some(JsonDecodeContext::String));
        assert_eq!((new.offset(), new.line(), new.column()), (7, 2, 3));
        assert_eq!(new.reason(), old.reason());
        assert_eq!(new.to_string(), old.to_string());
        assert_ne!(new, old);
    }

    #[test]
    fn test_eof_phase_survives_redacted_and_detailed_conversion() {
        let cases = [
            (r#"{"a":"x"#, JsonDecodeContext::String),
            (r#"{"a""#, JsonDecodeContext::Colon),
            (r#"{"a":1"#, JsonDecodeContext::ObjectSeparator),
            (r#"{"a":[1"#, JsonDecodeContext::ArraySeparator),
            (r#"{"a":-"#, JsonDecodeContext::Number),
        ];
        for diagnostics in [DiagnosticPolicy::Redacted, DiagnosticPolicy::Detailed] {
            for (text, expected) in cases {
                let options = JsonValidatedDecodeOptions::builder().build();
                let mut decoder = JsonDecoder::unlimited().with_diagnostic_policy(diagnostics);
                let error = decoder
                    .decode_unique_validated_str(text, &options, |_, _| Ok::<_, ()>(JsonNumberPolicy::Preserve))
                    .unwrap_err();
                let JsonValidatedDecodeError::Decode { error, context } = error else {
                    panic!("expected a structured decode error");
                };
                let syntax = error.syntax_error().unwrap();
                assert_eq!(syntax.reason(), JsonSyntaxErrorReason::UnexpectedEnd);
                assert_eq!(syntax.offset(), text.len());
                assert_eq!((syntax.line(), syntax.column()), (1, text.len() + 1));
                assert_eq!(syntax.context(), Some(expected));
                assert_eq!(context, syntax.context());
            }
        }
    }

    #[test]
    fn test_syntax_phase_tracks_nested_grammar_and_token_state() {
        let cases = [
            ("", JsonSyntaxErrorReason::UnexpectedEnd, JsonDecodeContext::Value),
            (
                "{x:1}",
                JsonSyntaxErrorReason::ExpectedObjectKey,
                JsonDecodeContext::ObjectKey,
            ),
            (
                "[{}x]",
                JsonSyntaxErrorReason::ExpectedCommaOrArrayEnd,
                JsonDecodeContext::ArraySeparator,
            ),
            ("[1,", JsonSyntaxErrorReason::UnexpectedEnd, JsonDecodeContext::Value),
            (
                "[\"中\"\r\nx]",
                JsonSyntaxErrorReason::ExpectedCommaOrArrayEnd,
                JsonDecodeContext::ArraySeparator,
            ),
            (
                "\"a\nb\"",
                JsonSyntaxErrorReason::UnexpectedByte,
                JsonDecodeContext::String,
            ),
            (
                r#"{"a":"x\"#,
                JsonSyntaxErrorReason::UnexpectedEnd,
                JsonDecodeContext::String,
            ),
            (
                r#"{"a":"\u12"#,
                JsonSyntaxErrorReason::UnexpectedEnd,
                JsonDecodeContext::String,
            ),
        ];
        for (text, reason, context) in cases {
            let mut decoder = JsonDecoder::unlimited();
            let error = decoder
                .decode_unique_validated_str(text, &JsonValidatedDecodeOptions::builder().build(), |_, _| {
                    Ok::<_, ()>(JsonNumberPolicy::Preserve)
                })
                .unwrap_err();
            let JsonValidatedDecodeError::Decode { error, context: outer } = error else {
                panic!("expected a structured decode error");
            };
            let syntax = error.syntax_error().unwrap();
            assert_eq!(syntax.reason(), reason, "input: {text:?}");
            assert_eq!(syntax.context(), Some(context), "input: {text:?}");
            assert_eq!(outer, Some(context), "input: {text:?}");
            if text == "[\"中\"\r\nx]" {
                assert_eq!((syntax.line(), syntax.column()), (2, 1));
            }
        }
    }

    #[test]
    fn test_typed_and_utf8_validation_keep_lexical_context() {
        let mut decoder = JsonDecoder::unlimited();
        let typed = decoder.decode_str::<serde_json::Value>("[1").unwrap_err();
        assert_eq!(
            typed.syntax_error().unwrap().context(),
            Some(JsonDecodeContext::ArraySeparator)
        );

        let validated = decoder.validate_utf8(b"{\"a\"").unwrap_err();
        assert_eq!(
            validated.syntax_error().unwrap().context(),
            Some(JsonDecodeContext::Colon)
        );
    }
}
