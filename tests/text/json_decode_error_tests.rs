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
use qubit_json::text::JsonDecodeError;
use qubit_json::text::JsonTextDecoder;
use serde_json::error::Category;

/// Verifies that strict text decoding uses the operation-specific error API.
#[test]
fn test_decoder_decodes_valid_slice() {
    let mut session =
        JsonDecodeSession::owned(JsonDecodeLimits::empty().with_max_nodes(4));
    let value: bool = JsonTextDecoder::new(&mut session)
        .decode(b"true")
        .expect("valid JSON decodes");
    assert!(value);
}

/// Verifies that strict typed failures do not expose serde input fragments.
#[test]
fn test_decoder_returns_safe_deserialize_metadata() {
    let mut session = JsonDecodeSession::owned(JsonDecodeLimits::empty());
    let error = JsonTextDecoder::new(&mut session)
        .decode::<u64>(br#""TOP_SECRET""#)
        .expect_err("a JSON string cannot deserialize into u64");

    assert!(matches!(
        error,
        JsonDecodeError::Deserialize {
            category: Category::Data,
            line: 1,
            column,
        } if column > 0
    ));
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
    let mut session = JsonDecodeSession::owned(
        JsonDecodeLimits::empty()
            .with_max_input_bytes(rejected.len() + accepted.len())
            .with_max_nodes(1),
    );

    assert!(matches!(
        JsonTextDecoder::<JsonResource, usize>::new(&mut session)
            .decode::<u64>(rejected),
        Err(JsonDecodeError::Deserialize { .. })
    ));
    assert_eq!(
        session
            .input_budget()
            .expect("configured input budget")
            .used(),
        rejected.len()
    );
    assert_eq!(session.value_budget().used_nodes(), Some(0));

    assert_eq!(
        JsonTextDecoder::<JsonResource, usize>::new(&mut session)
            .decode::<u64>(accepted)
            .expect("a new value must fit after typed rollback"),
        0
    );
    assert_eq!(session.value_budget().used_nodes(), Some(1));
}

/// Verifies strict lexical inspection commits valid input and rejects invalid
/// input without constructing a typed value.
#[test]
fn test_validate_accounts_document() {
    let mut session =
        JsonDecodeSession::owned(JsonDecodeLimits::empty().with_max_nodes(4));
    JsonTextDecoder::new(&mut session)
        .validate(br#"{"ok":true}"#)
        .expect("valid JSON should pass lexical inspection");
    assert_eq!(session.value_budget().used_nodes(), Some(2));
    let _ = JsonTextDecoder::new(&mut session)
        .validate(b"[")
        .expect_err("malformed JSON must be rejected");
}
