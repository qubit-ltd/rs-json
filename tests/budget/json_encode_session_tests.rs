// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests for JSON encode sessions.

use qubit_budget::ResourceBudget;
use qubit_budget::ResourceLimit;
use qubit_budget::StructureLimits;
use qubit_json::JsonEncodeLimits;
use qubit_json::JsonEncodeSession;
use qubit_json::JsonResource;
use qubit_json::JsonValueBudget;
use qubit_json::JsonValueLimits;
use qubit_json::encode_to_vec;

/// Verifies encode sessions expose their output-byte resource.
/// Verifies borrowed encode sessions charge caller-owned budgets in place.
#[test]
fn test_encode_session_exposes_only_output_resource() {
    let encode = JsonEncodeSession::owned(
        JsonEncodeLimits::empty().with_output_bytes_limit(ResourceLimit::new(
            JsonResource::OutputBytes,
            8,
        )),
    );

    assert_eq!(encode.max_output_bytes(), Some(8));
}

/// Verifies output-byte consumption is cumulative and atomic within one
/// session.
#[test]
fn test_encode_session_consumes_output_bytes_atomically() {
    let mut session = JsonEncodeSession::owned(
        JsonEncodeLimits::empty().with_output_bytes_limit(ResourceLimit::new(
            JsonResource::OutputBytes,
            3,
        )),
    );

    session.consume_output_bytes(3).expect("exact output fits");
    let error = session
        .consume_output_bytes(1)
        .expect_err("output budget is exhausted");
    assert_eq!(*error.resource(), JsonResource::OutputBytes);
}

/// Verifies encode sessions preserve every embedded JSON value limit.
#[test]
fn test_encode_session_preserves_embedded_value_limits() {
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
    let mut session = JsonEncodeSession::owned(
        JsonEncodeLimits::empty().with_value_limits(value_limits),
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

#[test]
fn test_encode_session_borrows_output_and_value_budgets() {
    let mut output = ResourceBudget::new(JsonResource::OutputBytes, 32);
    let mut value = JsonValueBudget::new(
        JsonValueLimits::empty().with_structure_limits(
            StructureLimits::empty()
                .with_nodes_limit(ResourceLimit::new(JsonResource::Nodes, 16)),
        ),
    );
    let mut session =
        JsonEncodeSession::borrowing(Some(&mut output), &mut value);

    let encoded =
        encode_to_vec(&serde_json::json!({"name": "qubit"}), &mut session)
            .expect("borrowed budgets should support online encoding");

    assert_eq!(encoded, br#"{"name":"qubit"}"#);
    assert_eq!(output.used(), encoded.len());
    assert!(value.structure_budget().used_nodes() > 0);
}
