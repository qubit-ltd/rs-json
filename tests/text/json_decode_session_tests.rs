// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests reusable JSON decode sessions.

use std::panic::AssertUnwindSafe;
use std::panic::catch_unwind;

use qubit_budget::ResourceBudget;
use qubit_budget::ResourceLimit;
use qubit_budget::StructureLimits;
use qubit_budget::json::JsonDecodeLimits;
use qubit_budget::json::JsonDecodeSession;
use qubit_budget::json::JsonEncodeLimits;
use qubit_budget::json::JsonEncodeSession;
use qubit_budget::json::JsonMeasurement;
use qubit_budget::json::JsonResource;
use qubit_budget::json::JsonValueBudget;
use qubit_budget::json::JsonValueLimits;
use qubit_json::lenient::LenientJsonDecoder;
use qubit_json::text::JsonTextDecoder;
use serde::de::IgnoredAny;

/// Verifies decode and encode sessions expose only their directional resources.
#[test]
fn decode_and_encode_sessions_have_independent_directional_resources() {
    let decode = JsonDecodeSession::owned(
        JsonDecodeLimits::<JsonResource, usize>::builder()
            .input_bytes_limit(ResourceLimit::new(JsonResource::InputBytes, 8))
            .build(),
    );
    let encode = JsonEncodeSession::owned(
        JsonEncodeLimits::<JsonResource, usize>::builder()
            .output_bytes_limit(ResourceLimit::new(
                JsonResource::OutputBytes,
                8,
            ))
            .build(),
    );

    assert_eq!(decode.max_input_bytes(), Some(8));
    assert_eq!(encode.max_output_bytes(), Some(8));
}

/// Verifies input-byte consumption is cumulative and atomic within one attempt.
#[test]
fn test_decode_attempt_consumes_input_bytes_atomically() {
    let mut session = JsonDecodeSession::owned(
        JsonDecodeLimits::<JsonResource, usize>::builder()
            .input_bytes_limit(ResourceLimit::new(JsonResource::InputBytes, 3))
            .build(),
    );

    let mut attempt = session.begin_value();
    attempt
        .try_consume_input_bytes(3)
        .expect("exact input fits");
    let error = attempt
        .try_consume_input_bytes(1)
        .expect_err("input budget is exhausted");
    assert_eq!(*error.resource(), JsonResource::InputBytes);
}

/// Verifies borrowing a session mutates caller-owned directional budgets.
#[test]
fn test_decode_session_borrowing_reuses_caller_owned_budgets() {
    let mut input = ResourceBudget::new(JsonResource::InputBytes, 16_usize);
    let mut value = JsonValueBudget::new(
        JsonValueLimits::<JsonResource, usize>::builder()
            .payload_bytes_limit(ResourceLimit::new(
                JsonResource::PayloadBytes,
                3_usize,
            ))
            .build(),
    );
    {
        let mut session =
            JsonDecodeSession::borrowing_input(&mut input, &mut value);
        JsonTextDecoder::new(&mut session)
            .decode::<IgnoredAny>(br#"{"a":1}"#)
            .expect("borrowed session should admit the document");
        assert_eq!(session.max_input_bytes(), Some(16_usize));
        assert_eq!(
            session.input_budget().map(|budget| budget.limit()),
            Some(16_usize)
        );
    }
    assert_eq!(input.remaining(), 9_usize);
    assert_eq!(value.used_payload_bytes(), Some(2_usize));
}

/// Verifies decode sessions preserve every embedded JSON value limit.
#[test]
fn test_decode_session_preserves_embedded_value_limits() {
    let value_limits = JsonValueLimits::<JsonResource, usize>::builder()
        .string_bytes_limit(ResourceLimit::new(JsonResource::StringBytes, 2))
        .payload_bytes_limit(ResourceLimit::new(JsonResource::PayloadBytes, 3))
        .structure_limits(
            StructureLimits::builder()
                .nodes_limit(ResourceLimit::new(JsonResource::Nodes, 2)),
        )
        .build();
    let mut session = JsonDecodeSession::owned(
        JsonDecodeLimits::<JsonResource, usize>::builder()
            .value_limits(value_limits)
            .build(),
    );

    let mut attempt = session.begin_value();
    attempt
        .try_admit(JsonMeasurement::String { depth: 1, bytes: 2 })
        .expect("exact string limit fits");
    assert_eq!(
        *attempt
            .try_admit(JsonMeasurement::String { depth: 1, bytes: 3 })
            .expect_err("overlong string fails")
            .resource(),
        JsonResource::StringBytes,
    );
    attempt
        .try_admit(JsonMeasurement::Number { depth: 1, bytes: 1 })
        .expect("exact payload limit fits");
    assert_eq!(
        *attempt
            .try_admit(JsonMeasurement::Key { bytes: 1 })
            .expect_err("exhausted payload fails")
            .resource(),
        JsonResource::PayloadBytes,
    );
    let _ = attempt
        .try_admit(JsonMeasurement::Null { depth: 1 })
        .expect_err("string and number already exhaust the node limit");
    attempt.commit();
    assert_eq!(session.value_budget().used_nodes(), Some(2));
    assert_eq!(session.value_budget().used_payload_bytes(), Some(3));
}

/// Verifies a rejected value attempt preserves earlier committed values while
/// raw input accounting remains cumulative across sequential documents.
#[test]
fn test_failed_second_value_preserves_first_commit_and_accumulates_input() {
    let first = b"null";
    let second = br#"[null,null]"#;
    let third = b"null";
    let mut session = JsonDecodeSession::owned(
        JsonDecodeLimits::<JsonResource, usize>::builder()
            .max_input_bytes(64)
            .max_nodes(3)
            .build(),
    );

    JsonTextDecoder::new(&mut session)
        .decode::<IgnoredAny>(first)
        .expect("first value must fit");
    assert!(
        JsonTextDecoder::new(&mut session)
            .decode::<IgnoredAny>(second)
            .is_err()
    );
    assert_eq!(session.value_budget().used_nodes(), Some(1));
    assert_eq!(
        session.input_budget().expect("input budget").used(),
        first.len() + second.len(),
    );

    JsonTextDecoder::new(&mut session)
        .decode::<IgnoredAny>(third)
        .expect("rolled-back second value must leave room for the third");
    assert_eq!(session.value_budget().used_nodes(), Some(2));
}

/// Verifies unwind preserves immediate input charges while rolling back staged
/// value accounting, leaving the session reusable.
#[test]
fn test_decode_attempt_panic_retains_input_and_reuses_value_capacity() {
    let mut session = JsonDecodeSession::owned(
        JsonDecodeLimits::<JsonResource, usize>::builder()
            .max_input_bytes(8)
            .max_nodes(1)
            .build(),
    );

    let result = catch_unwind(AssertUnwindSafe(|| {
        let mut attempt = session.begin_value();
        attempt
            .try_consume_input_bytes(4)
            .expect("input must fit before the panic");
        attempt
            .try_admit(JsonMeasurement::Null { depth: 1 })
            .expect("staged value must fit before the panic");
        panic!("intentional decode-attempt panic");
    }));

    assert!(result.is_err());
    assert_eq!(session.input_budget().expect("input budget").used(), 4,);
    assert_eq!(session.value_budget().used_nodes(), Some(0));
    JsonTextDecoder::new(&mut session)
        .decode::<IgnoredAny>(b"null")
        .expect("rolled-back value capacity must remain reusable");
    assert_eq!(session.value_budget().used_nodes(), Some(1));
}

/// Verifies lenient normalization and typed decode failures retain immediate
/// input charges but roll back staged values before the next attempt.
#[test]
fn test_lenient_typed_failure_retains_normalized_input_and_reuses_value_capacity()
 {
    let rejected = "```json\nnull\n```";
    let accepted = "null";
    let mut session = JsonDecodeSession::owned(
        JsonDecodeLimits::<JsonResource, usize>::builder()
            .max_input_bytes(rejected.len() + accepted.len())
            .max_normalized_input_bytes(8)
            .max_nodes(1)
            .build(),
    );
    let decoder = LenientJsonDecoder::default();

    assert!(
        decoder
            .decode_with_session::<u8>(rejected, &mut session)
            .is_err()
    );
    assert_eq!(
        session.input_budget().expect("input budget").used(),
        rejected.len(),
    );
    assert_eq!(
        session
            .normalized_input_budget()
            .expect("normalized input budget")
            .used(),
        accepted.len(),
    );
    assert_eq!(session.value_budget().used_nodes(), Some(0));

    decoder
        .decode_with_session::<IgnoredAny>(accepted, &mut session)
        .expect(
            "typed failure must leave value capacity for the next document",
        );
    assert_eq!(session.value_budget().used_nodes(), Some(1));
}
