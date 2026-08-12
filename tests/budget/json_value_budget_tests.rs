// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests for JSON value budget accounting.

use qubit_budget::ResourceLimit;
use qubit_budget::StructureLimits;
use qubit_json::JsonResource;
use qubit_json::JsonValueBudget;
use qubit_json::JsonValueLimits;

/// Verifies a cumulative payload maximum accepts an exact final increment.
#[test]
fn json_value_payload_limit_is_inclusive() {
    let limits = JsonValueLimits::empty().with_payload_bytes_limit(
        ResourceLimit::new(JsonResource::PayloadBytes, 3),
    );
    let mut budget = JsonValueBudget::new(limits);
    budget.consume_string_bytes(3).expect("exact payload fits");
    let error = budget
        .consume_string_bytes(1)
        .expect_err("one byte over fails");
    assert_eq!(*error.resource(), JsonResource::PayloadBytes);
}

/// Verifies a failed single-value point check does not consume shared payload.
#[test]
fn test_json_value_budget_rejects_single_value_before_payload_consumption() {
    let limits = JsonValueLimits::empty()
        .with_string_bytes_limit(ResourceLimit::new(
            JsonResource::StringBytes,
            3,
        ))
        .with_payload_bytes_limit(ResourceLimit::new(
            JsonResource::PayloadBytes,
            4,
        ));
    let mut budget = JsonValueBudget::new(limits);

    let error = budget
        .consume_string_bytes(4)
        .expect_err("overlong string must fail its point limit");
    assert_eq!(*error.resource(), JsonResource::StringBytes);
    budget
        .consume_number_bytes(4)
        .expect("rejected string must not consume payload");
}

/// Verifies keys, strings and numbers share one cumulative payload budget.
#[test]
fn test_json_value_budget_charges_keys_strings_and_numbers_to_shared_payload() {
    let limits = JsonValueLimits::empty().with_payload_bytes_limit(
        ResourceLimit::new(JsonResource::PayloadBytes, 6),
    );
    let mut budget = JsonValueBudget::new(limits);

    budget.consume_key_bytes(1).expect("key should fit");
    budget.consume_string_bytes(2).expect("string should fit");
    budget.consume_number_bytes(3).expect("number should fit");
    let error = budget
        .consume_key_bytes(1)
        .expect_err("payload is fully consumed");
    assert_eq!(*error.resource(), JsonResource::PayloadBytes);
}

/// Verifies a failed payload increment leaves its remaining capacity unchanged.
#[test]
fn test_json_value_budget_rejects_payload_increment_atomically() {
    let limits = JsonValueLimits::empty().with_payload_bytes_limit(
        ResourceLimit::new(JsonResource::PayloadBytes, 3),
    );
    let mut budget = JsonValueBudget::new(limits);

    budget
        .consume_string_bytes(2)
        .expect("initial payload fits");
    let error = budget
        .consume_number_bytes(2)
        .expect_err("two additional bytes exceed remaining payload");
    assert_eq!(*error.resource(), JsonResource::PayloadBytes);
    budget
        .consume_key_bytes(1)
        .expect("the failed increment must preserve the final payload byte");
}

/// Verifies structure checks delegate to the shared structural budget.
#[test]
fn test_json_value_budget_delegates_structure_checks_and_accumulates_nodes() {
    let limits = JsonValueLimits::empty().with_structure_limits(
        StructureLimits::empty()
            .with_depth_limit(ResourceLimit::new(JsonResource::Depth, 1))
            .with_nodes_limit(ResourceLimit::new(JsonResource::Nodes, 2))
            .with_sequence_items_limit(ResourceLimit::new(
                JsonResource::SequenceItems,
                1,
            )),
    );
    let mut budget = JsonValueBudget::new(limits);

    budget.enter_array(1, 1).expect("first array fits");
    budget.enter_node(1).expect("second node fits");
    assert_eq!(
        *budget
            .enter_node(1)
            .expect_err("nodes are cumulative")
            .resource(),
        JsonResource::Nodes,
    );
    assert_eq!(
        *budget
            .enter_array(2, 1)
            .expect_err("depth is a point limit")
            .resource(),
        JsonResource::Depth,
    );
}
