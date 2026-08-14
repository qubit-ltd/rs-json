// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use std::error::Error;

use qubit_budget::json::JsonDecodeLimits;
use qubit_budget::json::JsonDecodeSession;
use qubit_budget::json::JsonResource;
use qubit_json::text::JsonDecodeError;
use qubit_json::text::decode_slice;
use qubit_json::text::inspect;

/// Verifies that strict text decoding uses the operation-specific error API.
#[test]
fn test_decode_slice_returns_a_typed_value() {
    let mut session =
        JsonDecodeSession::owned(JsonDecodeLimits::empty().with_max_nodes(4));
    let value: bool =
        decode_slice(b"true", &mut session).expect("valid JSON decodes");
    assert!(value);
}

/// Verifies that strict typed failures do not expose serde input fragments.
#[test]
fn test_decode_slice_redacts_serde_input_details() {
    let mut session = JsonDecodeSession::owned(JsonDecodeLimits::empty());
    let error = decode_slice::<u64, _, _>(br#""TOP_SECRET""#, &mut session)
        .expect_err("a JSON string cannot deserialize into u64");

    assert!(matches!(error, JsonDecodeError::Deserialize(_)));
    assert!(!error.to_string().contains("TOP_SECRET"));
    assert!(error.source().is_none());
    assert!(!format!("{error:?}").contains("TOP_SECRET"));
}

/// Verifies strict text decoding retains input after a typed failure while
/// rolling back its admitted value before the session is reused.
#[test]
fn test_decode_slice_typed_failure_rolls_back_value_and_reuses_session() {
    let rejected = br#""not-a-number""#;
    let accepted = b"0";
    let mut session = JsonDecodeSession::owned(
        JsonDecodeLimits::empty()
            .with_max_input_bytes(rejected.len() + accepted.len())
            .with_max_nodes(1),
    );

    assert!(matches!(
        decode_slice::<u64, JsonResource, usize>(rejected, &mut session),
        Err(JsonDecodeError::Deserialize(_))
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
        decode_slice::<u64, JsonResource, usize>(accepted, &mut session)
            .expect("a new value must fit after typed rollback"),
        0
    );
    assert_eq!(session.value_budget().used_nodes(), Some(1));
}

/// Verifies strict lexical inspection commits valid input and rejects invalid
/// input without constructing a typed value.
#[test]
fn test_inspect_validates_and_accounts_document() {
    let mut session =
        JsonDecodeSession::owned(JsonDecodeLimits::empty().with_max_nodes(4));
    inspect(br#"{"ok":true}"#, &mut session)
        .expect("valid JSON should pass lexical inspection");
    assert_eq!(session.value_budget().used_nodes(), Some(2));
    let _ = inspect(b"[", &mut session)
        .expect_err("malformed JSON must be rejected");
}
