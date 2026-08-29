// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests strict JSON decoding errors through the public decoder API.
use std::error::Error;

use qubit_budget::json::JsonDecodeLimits;
use qubit_budget::json::JsonDecodeSession;
use qubit_budget::json::JsonResource;
use qubit_json::decode::JsonDecodeErrorKind;
use qubit_json::decode::JsonDecodeStage;
use qubit_json::decode::JsonDecoder;

/// Verifies that strict text decoding uses the operation-specific error API.
#[test]
fn test_decoder_decodes_valid_slice() {
    let session = JsonDecodeSession::from_limits(JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::builder().max_nodes(4).build());
    let value: bool = JsonDecoder::new(session)
        .decode_utf8(b"true")
        .expect("valid JSON decodes");
    assert!(value);
}

/// Verifies that strict typed failures do not expose serde input fragments.
#[test]
fn test_decoder_returns_safe_deserialize_metadata() {
    let session = JsonDecodeSession::from_limits(JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::builder().build());
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
        JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::builder()
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
    let session = JsonDecodeSession::from_limits(JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::builder().max_nodes(4).build());
    let mut decoder = JsonDecoder::new(session);
    decoder
        .validate_utf8(br#"{"ok":true}"#)
        .expect("valid JSON should pass lexical inspection");
    assert_eq!(decoder.session().value_budget().used_nodes(), Some(2));
    let _ = decoder
        .validate_utf8(b"[")
        .expect_err("malformed JSON must be rejected");
}
