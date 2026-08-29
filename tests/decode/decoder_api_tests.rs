// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Verifies the new strict and normalizing decoder entry points.

use qubit_budget::json::JsonDecodeLimits;
use qubit_budget::json::JsonDecodeSession;
use qubit_budget::json::JsonResource;
use qubit_json::decode::JsonDecodeErrorKind;
use qubit_json::decode::JsonDecoder;
use qubit_json::decode::MarkdownFencePolicy;
use qubit_json::decode::NormalizingJsonDecodePolicy;
use qubit_json::decode::NormalizingJsonDecoder;
use serde::de::IgnoredAny;

/// Creates a policy that exercises the normalization facade without rewriting.
fn no_normalization_policy() -> NormalizingJsonDecodePolicy {
    NormalizingJsonDecodePolicy::builder()
        .trim_whitespace(false)
        .strip_utf8_bom(false)
        .markdown_fence_policy(MarkdownFencePolicy::Disabled)
        .escape_control_chars_in_strings(false)
        .build()
}

/// Creates a reusable session with no configured resource limits.
/// Verifies that strict string decoding can borrow from the input.
#[test]
fn test_json_decoder_decode_str_borrows_input() {
    let mut decoder = JsonDecoder::unlimited();
    let input = "\"borrowed\"";

    let value: &str = decoder
        .decode_str(input)
        .expect("strict string decoding should succeed");

    assert_eq!(value, "borrowed");
}

/// Verifies that strict UTF-8 byte decoding can borrow from the input.
#[test]
fn test_json_decoder_decode_utf8_borrows_input() {
    let mut decoder = JsonDecoder::unlimited();
    let input = br#""borrowed""#;

    let value: &str = decoder
        .decode_utf8(input)
        .expect("strict UTF-8 decoding should succeed");

    assert_eq!(value, "borrowed");
}

/// Verifies that strict validation accepts string and UTF-8 entry points.
#[test]
fn test_json_decoder_validation_entry_points() {
    let mut decoder = JsonDecoder::unlimited();

    decoder
        .validate_str("null")
        .expect("strict string validation should succeed");
    decoder
        .validate_utf8(b"true")
        .expect("strict UTF-8 validation should succeed");
}

/// Verifies strict decoding exposes object and array top-level contracts for
/// both string and byte entry points.
#[test]
fn test_json_decoder_typed_root_entry_points() {
    let mut decoder = JsonDecoder::unlimited();

    let object: serde_json::Value = decoder.decode_object_str("{\"ok\":true}").expect("object string");
    let array: Vec<u8> = decoder.decode_array_utf8(b"[1,2]").expect("array bytes");
    let error = decoder
        .decode_object_utf8::<serde_json::Value>(b"[]")
        .expect_err("array must not satisfy object contract");

    assert_eq!(object["ok"], true);
    assert_eq!(array, [1, 2]);
    assert_eq!(error.kind(), JsonDecodeErrorKind::UnexpectedTopLevel);
}

/// Verifies both decoder families reject input containing more than one JSON
/// document.
#[test]
fn test_decoders_share_complete_document_admission() {
    let input = "null true";
    let mut strict = JsonDecoder::unlimited();
    let mut normalizing = NormalizingJsonDecoder::with_limits(no_normalization_policy(), qubit_budget::json::JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::default());

    assert!(strict.decode_str::<IgnoredAny>(input).is_err());
    assert!(normalizing.decode_str::<IgnoredAny>(input).is_err());
}

/// Verifies the owned constructor builds a cumulative session from explicit
/// limits.
#[test]
fn test_json_decoder_owned_uses_explicit_limits() {
    let limits = JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::builder()
        .max_input_bytes(8)
        .max_nodes(2)
        .build();
    let decoder = JsonDecoder::with_limits(limits);

    assert_eq!(decoder.session().max_input_bytes(), Some(8));
    assert_eq!(decoder.session().value_budget().limits().max_nodes(), Some(2));
}

/// Verifies unlimited construction is explicit and leaves every budget
/// unconfigured.
#[test]
fn test_json_decoder_unlimited_has_no_limits() {
    let decoder = JsonDecoder::unlimited();

    assert_eq!(decoder.session().max_input_bytes(), None);
    assert_eq!(decoder.session().value_budget().limits().max_nodes(), None);
}

/// Verifies that normalizing string decoding returns an owned target.
#[test]
fn test_normalizing_decoder_decode_str_returns_owned_value() {
    let mut decoder =
        NormalizingJsonDecoder::with_limits(NormalizingJsonDecodePolicy::lenient(), qubit_budget::json::JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::default());

    let value: String = decoder
        .decode_str("  \"owned\"  ")
        .expect("normalizing string decoding should succeed");

    assert_eq!(value, "owned");
}

#[test]
fn test_json_decoder_accumulates_owned_session_usage() {
    let limits = JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::builder()
        .max_input_bytes(16)
        .build();
    let mut decoder = JsonDecoder::new(JsonDecodeSession::from_limits(limits));
    decoder.decode_utf8::<bool>(b"true").expect("first value");
    decoder.decode_utf8::<bool>(b"false").expect("second value");
    assert_eq!(decoder.session().input_budget().unwrap().used(), 9);
}

/// Verifies that normalizing UTF-8 byte decoding returns an owned target.
#[test]
fn test_normalizing_decoder_decode_utf8_returns_owned_value() {
    let mut decoder =
        NormalizingJsonDecoder::with_limits(NormalizingJsonDecodePolicy::lenient(), qubit_budget::json::JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::default());

    let value: String = decoder
        .decode_utf8(br#"  "owned"  "#)
        .expect("normalizing UTF-8 decoding should succeed");

    assert_eq!(value, "owned");
}

/// Verifies that an owned decoder takes raw and normalized limits only from
/// `JsonDecodeLimits` while retaining an independent normalization policy.
#[test]
fn test_normalizing_decoder_owned_separates_policy_and_limits() {
    let policy = no_normalization_policy();
    let limits = JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::builder()
        .max_input_bytes(8)
        .max_normalized_input_bytes(6)
        .max_nodes(1)
        .build();
    let decoder = NormalizingJsonDecoder::with_limits(policy.clone(), limits);

    assert_eq!(decoder.policy(), &policy);
    assert_eq!(decoder.session().max_input_bytes(), Some(8));
    assert_eq!(decoder.session().max_normalized_input_bytes(), Some(6));
    assert_eq!(decoder.session().value_budget().limits().max_nodes(), Some(1));
}

/// Verifies that a caller-provided session remains the decoder's sole source
/// of limits and can be recovered after use.
#[test]
fn test_normalizing_decoder_new_preserves_session() {
    let limits = JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::builder()
        .max_input_bytes(9)
        .build();
    let session = JsonDecodeSession::from_limits(limits);
    let decoder = NormalizingJsonDecoder::new(NormalizingJsonDecodePolicy::lenient(), session);

    assert_eq!(decoder.session().max_input_bytes(), Some(9));
    assert_eq!(decoder.into_session().max_input_bytes(), Some(9));
}

/// Verifies normalization policy selection cannot change caller-owned session
/// limits.
#[test]
fn test_normalizing_decoder_policies_do_not_change_session_limits() {
    for policy in [no_normalization_policy(), NormalizingJsonDecodePolicy::lenient()] {
        let session = JsonDecodeSession::from_limits(
            JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::builder()
                .max_input_bytes(7)
                .max_normalized_input_bytes(5)
                .max_nodes(2)
                .build(),
        );
        let decoder = NormalizingJsonDecoder::new(policy, session);

        assert_eq!(decoder.session().max_input_bytes(), Some(7));
        assert_eq!(decoder.session().max_normalized_input_bytes(), Some(5));
        assert_eq!(decoder.session().value_budget().limits().max_nodes(), Some(2));
    }
}

/// Verifies a typed deserialization failure keeps immediate input charges and
/// rolls back staged value charges.
#[test]
fn test_normalizing_decoder_typed_failure_keeps_input_and_rolls_back_value() {
    let input = r#"{"flag":1}"#;
    let limits = JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::builder()
        .max_input_bytes(input.len())
        .max_normalized_input_bytes(input.len())
        .max_nodes(2)
        .build();
    let mut decoder = NormalizingJsonDecoder::with_limits(no_normalization_policy(), limits);

    let _ = decoder
        .decode_str::<std::collections::HashMap<String, bool>>(input)
        .expect_err("number should not deserialize as bool");

    assert_eq!(decoder.session().input_budget().unwrap().used(), input.len());
    assert_eq!(decoder.session().normalized_input_budget().unwrap().used(), input.len());
    assert_eq!(decoder.session().value_budget().used_nodes(), Some(0));
}
