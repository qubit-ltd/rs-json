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
use qubit_json::decode::JsonDecodeErrorKind;
use qubit_json::decode::JsonDecoder;
use qubit_json::decode::JsonSyntaxErrorReason;
use serde::Deserialize;
use serde::Deserializer;
use serde::de::DeserializeSeed;
use serde::de::Error as DeError;
use serde::de::IgnoredAny;

/// Verifies escaped and direct Unicode text consume the same decoded payload.
#[test]
fn test_escaped_and_direct_unicode_charge_equal_decoded_payload() {
    let limits = JsonDecodeLimits::<JsonResource, usize>::builder()
        .value_limits(
            JsonValueLimits::<JsonResource, usize>::builder()
                .payload_bytes_limit(ResourceLimit::new(JsonResource::PayloadBytes, 3))
                .build(),
        )
        .build();
    for input in [br#""\u4e2d""#.as_slice(), "\"中\"".as_bytes()] {
        let session = JsonDecodeSession::from_limits(limits);
        assert_eq!(
            JsonDecoder::new(session)
                .decode_utf8::<String>(input)
                .expect("three decoded UTF-8 bytes must fit"),
            "中"
        );
    }
}

/// Verifies lexical admission rejects excessive depth before typed
/// deserialization.
#[test]
fn test_deeply_nested_input_fails_by_limit_without_stack_overflow() {
    let input = format!("{}0{}", "[".repeat(20_000), "]".repeat(20_000));
    let session = JsonDecodeSession::from_limits(
        JsonDecodeLimits::<JsonResource, usize>::builder()
            .value_limits(
                JsonValueLimits::<JsonResource, usize>::builder()
                    .structure_limits(
                        StructureLimits::builder().depth_limit(ResourceLimit::new(JsonResource::Depth, 128)),
                    )
                    .build(),
            )
            .build(),
    );
    let error = JsonDecoder::new(session)
        .decode_utf8::<serde_json::Value>(input.as_bytes())
        .expect_err("the depth limit must reject the document");
    assert_eq!(error.kind(), JsonDecodeErrorKind::Budget);
}

#[test]
fn test_json_decode_reports_structured_syntax_locations() {
    let cases = [
        (b"".as_slice(), 0, 1, 1, JsonSyntaxErrorReason::UnexpectedEnd),
        (br#"{"a" 1}"#.as_slice(), 5, 1, 6, JsonSyntaxErrorReason::ExpectedColon),
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
        (br#""\x""#.as_slice(), 2, 1, 3, JsonSyntaxErrorReason::InvalidEscape),
        (br#"01"#.as_slice(), 1, 1, 2, JsonSyntaxErrorReason::InvalidNumber),
        (
            br#"true false"#.as_slice(),
            5,
            1,
            6,
            JsonSyntaxErrorReason::TrailingCharacters,
        ),
    ];
    for (input, offset, line, column, reason) in cases {
        let session = JsonDecodeSession::from_limits(JsonDecodeLimits::<JsonResource, usize>::builder().build());
        let error = JsonDecoder::new(session)
            .decode_utf8::<serde_json::Value>(input)
            .expect_err("input should be rejected");
        let error = error.syntax_error().expect("expected structured syntax error");
        assert_eq!(error.offset(), offset);
        assert_eq!(error.line(), line);
        assert_eq!(error.column(), column);
        assert_eq!(error.reason(), reason);
    }
}

#[test]
fn test_json_decode_counts_unicode_columns_and_crlf_lines() {
    let input = "{\r\n  \"中\" 1}".as_bytes();
    let session = JsonDecodeSession::from_limits(JsonDecodeLimits::<JsonResource, usize>::builder().build());
    let error = JsonDecoder::new(session)
        .decode_utf8::<serde_json::Value>(input)
        .expect_err("missing colon should be rejected");
    let error = error.syntax_error().expect("expected structured syntax error");
    assert_eq!(error.line(), 2);
    assert_eq!(error.column(), 7);
    assert_eq!(error.reason(), JsonSyntaxErrorReason::ExpectedColon);
}

/// Verifies typed decode failures still consume the attempted input bytes.
#[test]
fn test_typed_decode_failure_consumes_input_before_the_next_attempt() {
    let session = JsonDecodeSession::from_limits(
        JsonDecodeLimits::<JsonResource, usize>::builder()
            .input_bytes_limit(ResourceLimit::new(JsonResource::InputBytes, 3))
            .build(),
    );
    let mut decoder = JsonDecoder::new(session);

    let error = decoder
        .decode_utf8::<u8>(br#""x""#)
        .expect_err("the string must fail typed decoding");
    assert_eq!(error.kind(), JsonDecodeErrorKind::Deserialize);
    let error = decoder
        .decode_utf8::<u8>(b"0")
        .expect_err("the exhausted input budget must reject the next value");
    assert_eq!(error.kind(), JsonDecodeErrorKind::Budget);
}

/// Verifies a typed seed failure retains raw input while discarding staged
/// value admission.
#[test]
fn test_seed_rejection_keeps_input_and_rolls_back_value() {
    let input = br#"{"value":1}"#;
    let session = JsonDecodeSession::from_limits(
        JsonDecodeLimits::<JsonResource, usize>::builder()
            .max_input_bytes(64)
            .max_nodes(8)
            .build(),
    );
    let mut decoder = JsonDecoder::new(session);

    assert!(decoder.decode_seed_utf8(RejectSeed, input).is_err());
    assert_eq!(
        decoder
            .session()
            .input_budget()
            .expect("configured input budget")
            .used(),
        input.len()
    );
    assert_eq!(decoder.session().value_budget().used_nodes(), Some(0));
}

/// Verifies syntax rejection retains input but rolls back partial lexical
/// admission before a reusable session accepts the next value.
#[test]
fn test_syntax_rejection_rolls_back_value_and_reuses_session() {
    let rejected = br#"{"value":]"#;
    let accepted = br#"null"#;
    let session = JsonDecodeSession::from_limits(
        JsonDecodeLimits::<JsonResource, usize>::builder()
            .max_input_bytes(rejected.len() + accepted.len())
            .max_nodes(1)
            .build(),
    );
    let mut decoder = JsonDecoder::new(session);

    assert!(decoder.decode_utf8::<serde_json::Value>(rejected).is_err());
    assert_eq!(
        decoder
            .session()
            .input_budget()
            .expect("configured input budget")
            .used(),
        rejected.len()
    );
    assert_eq!(decoder.session().value_budget().used_nodes(), Some(0));

    assert_eq!(
        decoder
            .decode_utf8::<serde_json::Value>(accepted)
            .expect("a fresh value must fit after syntax rollback"),
        serde_json::Value::Null
    );
    assert_eq!(
        decoder
            .session()
            .input_budget()
            .expect("configured input budget")
            .used(),
        rejected.len() + accepted.len()
    );
    assert_eq!(decoder.session().value_budget().used_nodes(), Some(1));
}

/// Verifies value budget rejection retains the attempted input but never
/// publishes the rejected value's partial measurements.
#[test]
fn test_budget_rejection_keeps_input_and_rolls_back_value() {
    let input = br#"[null]"#;
    let session = JsonDecodeSession::from_limits(
        JsonDecodeLimits::<JsonResource, usize>::builder()
            .max_input_bytes(input.len())
            .max_nodes(1)
            .build(),
    );
    let mut decoder = JsonDecoder::new(session);

    let error = decoder
        .decode_utf8::<serde_json::Value>(input)
        .expect_err("the node budget must reject the document");
    assert_eq!(error.kind(), JsonDecodeErrorKind::Budget);
    assert_eq!(
        decoder
            .session()
            .input_budget()
            .expect("configured input budget")
            .used(),
        input.len()
    );
    assert_eq!(decoder.session().value_budget().used_nodes(), Some(0));
}

/// Verifies seed-first decoding rejects integers outside the public range.
#[test]
fn test_decoder_seed_rejects_integer_outside_64_bit_range() {
    let input = b"123456789012345678901234567890";
    let session = JsonDecodeSession::from_limits(
        JsonDecodeLimits::<JsonResource, usize>::builder()
            .value_limits(
                JsonValueLimits::<JsonResource, usize>::builder()
                    .number_bytes_limit(ResourceLimit::new(JsonResource::NumberBytes, input.len()))
                    .build(),
            )
            .build(),
    );
    let mut decoder = JsonDecoder::new(session);

    let error = decoder
        .decode_seed_utf8(IgnoreSeed, input)
        .expect_err("an integer above u64::MAX must be rejected");
    assert_eq!(error.kind(), JsonDecodeErrorKind::InvalidJson);
}

/// Verifies lexical limits reject input before a seed is invoked.
#[test]
fn test_point_limit_fails_before_seed_and_keeps_work_charged() {
    let limits = JsonDecodeLimits::<JsonResource, usize>::builder()
        .value_limits(
            JsonValueLimits::<JsonResource, usize>::builder()
                .structure_limits(StructureLimits::builder().nodes_limit(ResourceLimit::new(JsonResource::Nodes, 1)))
                .string_bytes_limit(ResourceLimit::new(JsonResource::StringBytes, 1))
                .build(),
        )
        .build();
    let session = JsonDecodeSession::from_limits(limits);
    let mut decoder = JsonDecoder::new(session);
    let error = decoder
        .decode_seed_utf8(PanicSeed, br#""ab""#)
        .expect_err("string limit must fail");
    assert_eq!(error.kind(), JsonDecodeErrorKind::Budget);
    assert_eq!(decoder.session().value_budget().used_nodes(), Some(0));
}

#[test]
fn test_decoder_supports_usize_quantities() {
    let limits = JsonDecodeLimits::<JsonResource, usize>::builder()
        .value_limits(
            JsonValueLimits::<JsonResource, usize>::builder()
                .structure_limits(
                    StructureLimits::builder().nodes_limit(ResourceLimit::new(JsonResource::Nodes, 4_usize)),
                )
                .build(),
        )
        .build();
    let session = JsonDecodeSession::<JsonResource, usize>::from_limits(limits);
    let mut decoder = JsonDecoder::new(session);

    let value: serde_json::Value = decoder
        .decode_utf8(br#"[1,"x"]"#)
        .expect("usize JSON budgets must admit a fitting document");
    assert_eq!(decoder.session().value_budget().used_nodes(), Some(3));
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
