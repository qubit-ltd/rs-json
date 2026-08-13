// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests for JSON decode sessions.

use qubit_budget::ResourceBudget;
use qubit_budget::ResourceLimit;
use qubit_budget::StructureLimits;
use qubit_budget::json::JsonDecodeLimits;
use qubit_budget::json::JsonDecodeSession;
use qubit_budget::json::JsonEncodeLimits;
use qubit_budget::json::JsonEncodeSession;
use qubit_budget::json::JsonResource;
use qubit_budget::json::JsonValueBudget;
use qubit_budget::json::JsonValueLimits;
use qubit_json::text::decode_slice;
use serde::de::IgnoredAny;

/// Verifies decode and encode sessions expose only their directional resources.
#[test]
fn decode_and_encode_sessions_have_independent_directional_resources() {
    let decode = JsonDecodeSession::owned(
        JsonDecodeLimits::empty().with_input_bytes_limit(ResourceLimit::new(
            JsonResource::InputBytes,
            8,
        )),
    );
    let encode = JsonEncodeSession::owned(
        JsonEncodeLimits::empty().with_output_bytes_limit(ResourceLimit::new(
            JsonResource::OutputBytes,
            8,
        )),
    );

    assert_eq!(decode.max_input_bytes(), Some(8));
    assert_eq!(encode.max_output_bytes(), Some(8));
}

/// Verifies input-byte consumption is cumulative and atomic within one session.
#[test]
fn test_decode_session_consumes_input_bytes_atomically() {
    let mut session = JsonDecodeSession::owned(
        JsonDecodeLimits::empty().with_input_bytes_limit(ResourceLimit::new(
            JsonResource::InputBytes,
            3,
        )),
    );

    session.consume_input_bytes(3).expect("exact input fits");
    let error = session
        .consume_input_bytes(1)
        .expect_err("input budget is exhausted");
    assert_eq!(*error.resource(), JsonResource::InputBytes);
}

/// Verifies borrowing a session mutates caller-owned directional budgets.
#[test]
fn test_decode_session_borrowing_reuses_caller_owned_budgets() {
    let mut input = ResourceBudget::new(JsonResource::InputBytes, 16_usize);
    let mut value = JsonValueBudget::new(
        JsonValueLimits::empty().with_payload_bytes_limit(ResourceLimit::new(
            JsonResource::PayloadBytes,
            3_usize,
        )),
    );
    {
        let mut session =
            JsonDecodeSession::borrowing_input(&mut input, &mut value);
        decode_slice::<IgnoredAny, _, _>(br#"{"a":1}"#, &mut session)
            .expect("borrowed session should admit the document");
        assert_eq!(session.max_input_bytes(), Some(16_usize));
        assert_eq!(
            session.input_budget().map(|budget| budget.limit()),
            Some(16_usize)
        );
    }
    assert_eq!(input.remaining(), 9_usize);
    assert_eq!(value.payload_budget().unwrap().remaining(), 1_usize);
}

/// Verifies decode sessions preserve every embedded JSON value limit.
#[test]
fn test_decode_session_preserves_embedded_value_limits() {
    let value_limits = JsonValueLimits::empty()
        .with_string_bytes_limit(ResourceLimit::new(
            JsonResource::StringBytes,
            2,
        ))
        .with_payload_bytes_limit(ResourceLimit::new(
            JsonResource::PayloadBytes,
            3,
        ))
        .with_structure_limits(
            StructureLimits::empty()
                .with_nodes_limit(ResourceLimit::new(JsonResource::Nodes, 1)),
        );
    let mut session = JsonDecodeSession::owned(
        JsonDecodeLimits::empty().with_value_limits(value_limits),
    );

    session
        .value_budget_mut()
        .consume_string_bytes(2)
        .expect("exact string limit fits");
    assert_eq!(
        *session
            .value_budget_mut()
            .consume_string_bytes(3)
            .expect_err("overlong string fails")
            .resource(),
        JsonResource::StringBytes,
    );
    session
        .value_budget_mut()
        .consume_number_bytes(1)
        .expect("exact payload limit fits");
    assert_eq!(
        *session
            .value_budget_mut()
            .consume_key_bytes(1)
            .expect_err("exhausted payload fails")
            .resource(),
        JsonResource::PayloadBytes,
    );
    session
        .value_budget_mut()
        .enter_node(1)
        .expect("exact node limit fits");
    assert_eq!(
        *session
            .value_budget_mut()
            .enter_node(1)
            .expect_err("exhausted node limit fails")
            .resource(),
        JsonResource::Nodes,
    );
}
