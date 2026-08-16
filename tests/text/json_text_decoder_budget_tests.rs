// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests strict JSON decoding admission and session accounting.
// qubit-style: allow explicit-imports

use qubit_budget::ResourceLimit;
use qubit_budget::StructureLimits;
use qubit_budget::json::JsonDecodeLimits;
use qubit_budget::json::JsonDecodeSession;
use qubit_budget::json::JsonResource;
use qubit_budget::json::JsonValueLimits;
use qubit_json::text::JsonDecodeError;
use qubit_json::text::JsonSyntaxErrorReason;
use qubit_json::text::JsonTextDecoder;
use serde::Deserialize;
use serde::Deserializer;
use serde::de::DeserializeSeed;
use serde::de::Error as DeError;
use serde::de::IgnoredAny;

/// Verifies escaped and direct Unicode text consume the same decoded payload.
#[test]
fn escaped_and_direct_unicode_charge_equal_decoded_payload() {
    let limits = JsonDecodeLimits::<JsonResource, usize>::builder()
        .value_limits(
            JsonValueLimits::<JsonResource, usize>::builder()
                .payload_bytes_limit(ResourceLimit::new(
                    JsonResource::PayloadBytes,
                    3,
                ))
                .build(),
        )
        .build();
    for input in [br#""\u4e2d""#.as_slice(), "\"中\"".as_bytes()] {
        let mut session = JsonDecodeSession::owned(limits);
        assert_eq!(
            JsonTextDecoder::new(&mut session)
                .decode::<String>(input)
                .expect("three decoded UTF-8 bytes must fit"),
            "中"
        );
    }
}

/// Verifies lexical admission rejects excessive depth before typed
/// deserialization.
#[test]
fn deeply_nested_input_fails_by_limit_without_stack_overflow() {
    let input = format!("{}0{}", "[".repeat(20_000), "]".repeat(20_000));
    let mut session = JsonDecodeSession::owned(
        JsonDecodeLimits::<JsonResource, usize>::builder()
            .value_limits(
                JsonValueLimits::<JsonResource, usize>::builder()
                    .structure_limits(StructureLimits::builder().depth_limit(
                        ResourceLimit::new(JsonResource::Depth, 128),
                    ))
                    .build(),
            )
            .build(),
    );

    assert!(matches!(
        JsonTextDecoder::new(&mut session)
            .decode::<serde_json::Value>(input.as_bytes()),
        Err(JsonDecodeError::Budget(_))
    ));
}

#[test]
fn json_decode_reports_structured_syntax_locations() {
    let cases = [
        (
            b"".as_slice(),
            0,
            1,
            1,
            JsonSyntaxErrorReason::UnexpectedEnd,
        ),
        (
            br#"{"a" 1}"#.as_slice(),
            5,
            1,
            6,
            JsonSyntaxErrorReason::ExpectedColon,
        ),
        (
            br#"[1 2]"#.as_slice(),
            3,
            1,
            4,
            JsonSyntaxErrorReason::ExpectedCommaOrArrayEnd,
        ),
        (
            br#"{"a":1 "b":2}"#.as_slice(),
            7,
            1,
            8,
            JsonSyntaxErrorReason::ExpectedCommaOrObjectEnd,
        ),
        (
            br#""\x""#.as_slice(),
            2,
            1,
            3,
            JsonSyntaxErrorReason::InvalidEscape,
        ),
        (
            br#"01"#.as_slice(),
            1,
            1,
            2,
            JsonSyntaxErrorReason::InvalidNumber,
        ),
        (
            br#"true false"#.as_slice(),
            5,
            1,
            6,
            JsonSyntaxErrorReason::TrailingCharacters,
        ),
    ];
    for (input, offset, line, column, reason) in cases {
        let mut session = JsonDecodeSession::owned(
            JsonDecodeLimits::<JsonResource, usize>::builder().build(),
        );
        let error = JsonTextDecoder::new(&mut session)
            .decode::<serde_json::Value>(input)
            .expect_err("input should be rejected");
        let JsonDecodeError::Syntax(error) = error else {
            panic!("expected structured syntax error");
        };
        assert_eq!(error.offset(), offset);
        assert_eq!(error.line(), line);
        assert_eq!(error.column(), column);
        assert_eq!(error.reason(), reason);
    }
}

#[test]
fn json_decode_counts_unicode_columns_and_crlf_lines() {
    let input = "{\r\n  \"中\" 1}".as_bytes();
    let mut session = JsonDecodeSession::owned(
        JsonDecodeLimits::<JsonResource, usize>::builder().build(),
    );
    let error = JsonTextDecoder::new(&mut session)
        .decode::<serde_json::Value>(input)
        .expect_err("missing colon should be rejected");
    let JsonDecodeError::Syntax(error) = error else {
        panic!("expected structured syntax error");
    };
    assert_eq!(error.line(), 2);
    assert_eq!(error.column(), 7);
    assert_eq!(error.reason(), JsonSyntaxErrorReason::ExpectedColon);
}

/// Verifies typed decode failures still consume the attempted input bytes.
#[test]
fn typed_decode_failure_consumes_input_before_the_next_attempt() {
    let mut session = JsonDecodeSession::owned(
        JsonDecodeLimits::<JsonResource, usize>::builder()
            .input_bytes_limit(ResourceLimit::new(JsonResource::InputBytes, 3))
            .build(),
    );

    assert!(matches!(
        JsonTextDecoder::new(&mut session).decode::<u8>(br#""x""#),
        Err(JsonDecodeError::Deserialize { .. })
    ));
    assert!(matches!(
        JsonTextDecoder::new(&mut session).decode::<u8>(b"0"),
        Err(JsonDecodeError::Budget(_))
    ));
}

/// Verifies a typed seed failure retains raw input while discarding staged
/// value admission.
#[test]
fn seed_rejection_keeps_input_and_rolls_back_value() {
    let input = br#"{"value":1}"#;
    let mut session = JsonDecodeSession::owned(
        JsonDecodeLimits::<JsonResource, usize>::builder()
            .max_input_bytes(64)
            .max_nodes(8)
            .build(),
    );

    assert!(
        JsonTextDecoder::new(&mut session)
            .decode_seed(RejectSeed, input)
            .is_err()
    );
    assert_eq!(
        session
            .input_budget()
            .expect("configured input budget")
            .used(),
        input.len()
    );
    assert_eq!(session.value_budget().used_nodes(), Some(0));
}

/// Verifies syntax rejection retains input but rolls back partial lexical
/// admission before a reusable session accepts the next value.
#[test]
fn syntax_rejection_rolls_back_value_and_reuses_session() {
    let rejected = br#"{"value":]"#;
    let accepted = br#"null"#;
    let mut session = JsonDecodeSession::owned(
        JsonDecodeLimits::<JsonResource, usize>::builder()
            .max_input_bytes(rejected.len() + accepted.len())
            .max_nodes(1)
            .build(),
    );

    assert!(
        JsonTextDecoder::new(&mut session)
            .decode::<serde_json::Value>(rejected)
            .is_err()
    );
    assert_eq!(
        session
            .input_budget()
            .expect("configured input budget")
            .used(),
        rejected.len()
    );
    assert_eq!(session.value_budget().used_nodes(), Some(0));

    assert_eq!(
        JsonTextDecoder::new(&mut session)
            .decode::<serde_json::Value>(accepted)
            .expect("a fresh value must fit after syntax rollback"),
        serde_json::Value::Null
    );
    assert_eq!(
        session
            .input_budget()
            .expect("configured input budget")
            .used(),
        rejected.len() + accepted.len()
    );
    assert_eq!(session.value_budget().used_nodes(), Some(1));
}

/// Verifies value budget rejection retains the attempted input but never
/// publishes the rejected value's partial measurements.
#[test]
fn budget_rejection_keeps_input_and_rolls_back_value() {
    let input = br#"[null]"#;
    let mut session = JsonDecodeSession::owned(
        JsonDecodeLimits::<JsonResource, usize>::builder()
            .max_input_bytes(input.len())
            .max_nodes(1)
            .build(),
    );

    assert!(matches!(
        JsonTextDecoder::new(&mut session).decode::<serde_json::Value>(input),
        Err(JsonDecodeError::Budget(_))
    ));
    assert_eq!(
        session
            .input_budget()
            .expect("configured input budget")
            .used(),
        input.len()
    );
    assert_eq!(session.value_budget().used_nodes(), Some(0));
}

/// Verifies seed-first decoding uses the same lexical admission path.
#[test]
fn decoder_seed_admits_arbitrary_precision_numbers() {
    let input = b"123456789012345678901234567890";
    let mut session = JsonDecodeSession::owned(
        JsonDecodeLimits::<JsonResource, usize>::builder()
            .value_limits(
                JsonValueLimits::<JsonResource, usize>::builder()
                    .number_bytes_limit(ResourceLimit::new(
                        JsonResource::NumberBytes,
                        input.len(),
                    ))
                    .build(),
            )
            .build(),
    );

    JsonTextDecoder::new(&mut session)
        .decode_seed(IgnoreSeed, input)
        .expect("the exact arbitrary-precision lexical number limit must fit");
}

/// Verifies lexical limits reject input before a seed is invoked.
#[test]
fn point_limit_fails_before_seed_and_keeps_work_charged() {
    let limits =
        JsonDecodeLimits::<JsonResource, usize>::builder()
            .value_limits(
                JsonValueLimits::<JsonResource, usize>::builder()
                    .structure_limits(StructureLimits::builder().nodes_limit(
                        ResourceLimit::new(JsonResource::Nodes, 1),
                    ))
                    .string_bytes_limit(ResourceLimit::new(
                        JsonResource::StringBytes,
                        1,
                    ))
                    .build(),
            )
            .build();
    let mut session = JsonDecodeSession::owned(limits);
    let error = JsonTextDecoder::new(&mut session)
        .decode_seed(PanicSeed, br#""ab""#)
        .expect_err("string limit must fail");
    assert!(matches!(error, JsonDecodeError::Budget(_)));
    assert_eq!(session.value_budget().used_nodes(), Some(0));
}

#[test]
fn decoder_supports_usize_quantities() {
    let limits = JsonDecodeLimits::<JsonResource, usize>::builder()
        .value_limits(
            JsonValueLimits::<JsonResource, usize>::builder()
                .structure_limits(StructureLimits::builder().nodes_limit(
                    ResourceLimit::new(JsonResource::Nodes, 4_usize),
                ))
                .build(),
        )
        .build();
    let mut session = JsonDecodeSession::<JsonResource, usize>::owned(limits);

    let value: serde_json::Value = JsonTextDecoder::new(&mut session)
        .decode(br#"[1,"x"]"#)
        .expect("usize JSON budgets must admit a fitting document");
    assert_eq!(session.value_budget().used_nodes(), Some(3));
    assert!(value.is_array());
}

struct PanicSeed;

impl<'de> DeserializeSeed<'de> for PanicSeed {
    type Value = ();

    fn deserialize<D>(self, _: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        panic!("seed must not run before lexical admission succeeds");
    }
}

/// Seed which first accepts the JSON value then rejects its typed result.
struct RejectSeed;

impl<'de> DeserializeSeed<'de> for RejectSeed {
    type Value = ();

    /// Rejects the parsed JSON value after complete syntax admission.
    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        let _ = serde_json::Value::deserialize(deserializer)?;
        Err(DeError::custom("reject after JSON admission"))
    }
}

/// Seed that accepts one complete value without constructing a typed payload.
struct IgnoreSeed;

impl<'de> DeserializeSeed<'de> for IgnoreSeed {
    type Value = ();

    /// Ignores the admitted JSON value produced by the supplied deserializer.
    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        IgnoredAny::deserialize(deserializer).map(|_| ())
    }
}
