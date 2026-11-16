// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests strict JSON decoding errors through the public decoder API.
use std::collections::BTreeMap;
use std::error::Error;

use qubit_budget::json::JsonDecodeLimits;
use qubit_budget::json::JsonDecodeSession;
use qubit_budget::json::JsonResource;
use qubit_json::decode::DiagnosticPolicy;
use qubit_json::decode::JsonDecodeErrorKind;
use qubit_json::decode::JsonDecodeStage;
use qubit_json::decode::JsonDecoder;

/// Verifies syntax columns count Unicode scalar values while offsets count
/// bytes.
#[test]
fn test_syntax_column_counts_unicode_scalars() {
    let input = "{\"名\" 0}";
    let error = JsonDecoder::unlimited().validate_str(input).unwrap_err();
    let syntax = error.syntax_error().unwrap();
    let offset = input.find('0').unwrap();

    assert_eq!(syntax.offset(), offset);
    assert_eq!(error.line(), Some(1));
    assert_eq!(error.column(), Some(input[..offset].chars().count() + 1));
    assert_ne!(error.column(), Some(offset + 1));
}

/// Verifies deserialize columns preserve serde_json byte coordinates.
#[test]
fn test_deserialize_column_preserves_backend_byte_units() {
    let input = "{\"名\":\"坏\"}";
    let backend = serde_json::from_str::<BTreeMap<String, u8>>(input).unwrap_err();
    let error = JsonDecoder::unlimited()
        .decode_str::<BTreeMap<String, u8>>(input)
        .unwrap_err();
    let backend_column = backend.column();

    assert_eq!(error.kind(), JsonDecodeErrorKind::Deserialize);
    assert_eq!(error.line(), Some(backend.line()));
    assert_eq!(error.column(), Some(backend_column));
    let character_count_before_column = input
        .char_indices()
        .take_while(|(byte_index, _)| *byte_index < backend_column)
        .count();
    assert_ne!(backend_column, character_count_before_column);
}

/// Verifies scanner line coordinates for LF, CRLF, and standalone CR.
#[test]
fn test_syntax_coordinates_recognize_line_endings() {
    for input in ["{\r\n\"名\" 0}", "{\n\"名\" 0}", "{\r\"名\" 0}"] {
        let error = JsonDecoder::unlimited().validate_str(input).unwrap_err();
        let syntax = error.syntax_error().unwrap();
        let offset = input.find('0').unwrap();
        let line_start = input[..offset].rfind(['\r', '\n']).map_or(0, |i| i + 1);

        assert_eq!(error.line(), Some(2));
        assert_eq!(error.column(), Some(input[line_start..offset].chars().count() + 1));
        assert_eq!(syntax.offset(), offset);
    }
}

/// Verifies backend line coordinates and redaction for each line ending.
#[test]
fn test_deserialize_coordinates_and_redaction_recognize_line_endings() {
    const SECRET: &str = "private-value";
    for input in [
        "{\r\n\"名\":\"private-value\"}",
        "{\n\"名\":\"private-value\"}",
        "{\r\"名\":\"private-value\"}",
    ] {
        let backend = serde_json::from_str::<BTreeMap<String, u8>>(input).unwrap_err();
        let error = JsonDecoder::unlimited()
            .decode_str::<BTreeMap<String, u8>>(input)
            .unwrap_err();

        assert_eq!(error.kind(), JsonDecodeErrorKind::Deserialize);
        assert_eq!(error.line(), Some(backend.line()));
        assert_eq!(error.column(), Some(backend.column()));
        assert!(!error.to_string().contains(SECRET));
    }
}

/// Verifies strict decoders preserve the existing redacted default.
#[test]
fn test_json_decoder_defaults_to_redacted_diagnostics() {
    let decoder = JsonDecoder::unlimited();

    assert_eq!(decoder.diagnostic_policy(), DiagnosticPolicy::Redacted);
}

/// Verifies detailed strict decoding retains each input-derived error source.
#[test]
fn test_json_decoder_detailed_diagnostics_preserve_sources() {
    let mut decoder = JsonDecoder::unlimited().with_diagnostic_policy(DiagnosticPolicy::Detailed);

    let invalid_utf8 = decoder
        .validate_utf8(&[0xff])
        .expect_err("invalid UTF-8 must be rejected");
    assert_eq!(invalid_utf8.diagnostic_policy(), DiagnosticPolicy::Detailed);
    assert!(invalid_utf8.source().is_some());

    let invalid_json = decoder.validate_str("{").expect_err("incomplete JSON must be rejected");
    assert_eq!(invalid_json.diagnostic_policy(), DiagnosticPolicy::Detailed);
    assert!(invalid_json.source().is_some());

    let deserialize = decoder
        .decode_str::<u64>(r#""TOP_SECRET""#)
        .expect_err("a JSON string cannot deserialize into u64");
    assert_eq!(deserialize.diagnostic_policy(), DiagnosticPolicy::Detailed);
    assert!(deserialize.source().is_some());
}

/// Verifies detailed diagnostics also reach typed top-level checks.
#[test]
fn test_json_decoder_detailed_diagnostics_cover_typed_root_checks() {
    let mut decoder = JsonDecoder::unlimited().with_diagnostic_policy(DiagnosticPolicy::Detailed);

    let object_error = decoder
        .decode_object_str::<serde_json::Value>("[]")
        .expect_err("an array must not satisfy the object contract");
    assert_eq!(object_error.diagnostic_policy(), DiagnosticPolicy::Detailed);

    let array_error = decoder
        .decode_array_str::<serde_json::Value>("{}")
        .expect_err("an object must not satisfy the array contract");
    assert_eq!(array_error.diagnostic_policy(), DiagnosticPolicy::Detailed);
}

/// Verifies that strict text decoding uses the operation-specific error API.
#[test]
fn test_decoder_decodes_valid_slice() {
    let session =
        JsonDecodeSession::from_limits(JsonDecodeLimits::<JsonResource, usize>::builder().max_nodes(4).build());
    let value: bool = JsonDecoder::new(session)
        .decode_utf8(b"true")
        .expect("valid JSON decodes");
    assert!(value);
}

/// Verifies that strict typed failures do not expose serde input fragments.
#[test]
fn test_decoder_returns_safe_deserialize_metadata() {
    let session = JsonDecodeSession::from_limits(JsonDecodeLimits::<JsonResource, usize>::builder().build());
    let error = JsonDecoder::new(session)
        .decode_utf8::<u64>(br#""TOP_SECRET""#)
        .expect_err("a JSON string cannot deserialize into u64");

    assert_eq!(error.kind(), JsonDecodeErrorKind::Deserialize);
    assert_eq!(error.stage(), JsonDecodeStage::Deserialize);
    assert_eq!(error.line(), Some(1));
    assert!(error.column().is_some_and(|column| column > 0));
    assert!(!error.to_string().contains("TOP_SECRET"));
    assert!(error.source().is_none());
    assert!(!format!("{error:?}").contains("TOP_SECRET"));
}

/// Verifies strict text decoding retains input after a typed failure while
/// rolling back its admitted value before the session is reused.
#[test]
fn test_decoder_typed_failure_rolls_back_value_and_reuses_session() {
    let rejected = br#""not-a-number""#;
    let accepted = b"0";
    let session = JsonDecodeSession::from_limits(
        JsonDecodeLimits::<JsonResource, usize>::builder()
            .max_input_bytes(rejected.len() + accepted.len())
            .max_nodes(1)
            .build(),
    );

    let mut decoder = JsonDecoder::<JsonResource, usize>::new(session);
    let error = decoder
        .decode_utf8::<u64>(rejected)
        .expect_err("the rejected value must fail typed decoding");
    assert_eq!(error.kind(), JsonDecodeErrorKind::Deserialize);
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
            .decode_utf8::<u64>(accepted)
            .expect("a new value must fit after typed rollback"),
        0
    );
    assert_eq!(decoder.session().value_budget().used_nodes(), Some(1));
}

/// Verifies strict lexical inspection commits valid input and rejects invalid
/// input without constructing a typed value.
#[test]
fn test_validate_accounts_document() {
    let session =
        JsonDecodeSession::from_limits(JsonDecodeLimits::<JsonResource, usize>::builder().max_nodes(4).build());
    let mut decoder = JsonDecoder::new(session);
    decoder
        .validate_utf8(br#"{"ok":true}"#)
        .expect("valid JSON should pass lexical inspection");
    assert_eq!(decoder.session().value_budget().used_nodes(), Some(2));
    let _ = decoder
        .validate_utf8(b"[")
        .expect_err("malformed JSON must be rejected");
}
