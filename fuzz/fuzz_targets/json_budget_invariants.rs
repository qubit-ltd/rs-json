// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Fuzzes JSON session accounting invariants through the public API.

#![no_main]

use libfuzzer_sys::fuzz_target;
use qubit_budget::ResourceBudget;
use qubit_budget::ResourceLimit;
use qubit_budget::StructureLimits;
use qubit_budget::json::JsonDecodeSession;
use qubit_budget::json::JsonResource;
use qubit_budget::json::JsonValueBudget;
use qubit_budget::json::JsonValueLimits;
use qubit_json::text::decode_slice;
use serde_json::Value;

#[path = "../../tests/fixtures/internal/fuzz_limit.rs"]
mod fuzz_limit;

const MAX_INPUT_LEN: usize = 4 * 1024;

fuzz_target!(|data: &[u8]| {
    let input = &data[..data.len().min(MAX_INPUT_LEN)];
    let bytes = fuzz_limit::limit(data, 0);
    let depth = fuzz_limit::limit(data, 2);
    let nodes = fuzz_limit::limit(data, 4);
    let items = fuzz_limit::limit(data, 6);
    let entries = fuzz_limit::limit(data, 8);
    let key_bytes = fuzz_limit::limit(data, 10);
    let string_bytes = fuzz_limit::limit(data, 12);
    let number_bytes = fuzz_limit::limit(data, 14);
    let payload_bytes = fuzz_limit::limit(data, 16);

    let structure = StructureLimits::empty()
        .with_depth_limit(ResourceLimit::new(JsonResource::Depth, depth))
        .with_nodes_limit(ResourceLimit::new(JsonResource::Nodes, nodes))
        .with_sequence_items_limit(ResourceLimit::new(
            JsonResource::SequenceItems,
            items,
        ))
        .with_map_entries_limit(ResourceLimit::new(
            JsonResource::MapEntries,
            entries,
        ))
        .with_key_bytes_limit(ResourceLimit::new(
            JsonResource::KeyBytes,
            key_bytes,
        ));
    let value_limits = JsonValueLimits::empty()
        .with_structure_limits(structure)
        .with_string_bytes_limit(ResourceLimit::new(
            JsonResource::StringBytes,
            string_bytes,
        ))
        .with_number_bytes_limit(ResourceLimit::new(
            JsonResource::NumberBytes,
            number_bytes,
        ))
        .with_payload_bytes_limit(ResourceLimit::new(
            JsonResource::PayloadBytes,
            payload_bytes,
        ));
    let mut input_budget = ResourceBudget::new(JsonResource::InputBytes, bytes);
    let mut value_budget = JsonValueBudget::new(value_limits);
    {
        let mut session = JsonDecodeSession::borrowing_input(
            &mut input_budget,
            &mut value_budget,
        );
        let _ = decode_slice::<Value, _, _>(input, &mut session);
        assert_eq!(session.max_input_bytes(), Some(bytes));
        assert!(
            session.value_budget().structure_budget().used_nodes() <= nodes
        );
    }

    assert!(input_budget.used() <= bytes);
    assert_eq!(input_budget.used() + input_budget.remaining(), bytes);
    assert!(value_budget.structure_budget().used_nodes() <= nodes);
});
