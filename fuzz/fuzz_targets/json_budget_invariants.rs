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

const MAX_INPUT_LEN: usize = 4 * 1024;
const MAX_LIMIT: usize = 4 * 1024;

fn limit(data: u8) -> usize {
    1 + usize::from(data) % MAX_LIMIT
}

fuzz_target!(|data: &[u8]| {
    let input = &data[..data.len().min(MAX_INPUT_LEN)];
    let bytes = data.get(0).copied().map(limit).unwrap_or(1);
    let depth = data.get(1).copied().map(limit).unwrap_or(1);
    let nodes = data.get(2).copied().map(limit).unwrap_or(1);
    let items = data.get(3).copied().map(limit).unwrap_or(1);
    let entries = data.get(4).copied().map(limit).unwrap_or(1);
    let key_bytes = data.get(5).copied().map(limit).unwrap_or(1);
    let string_bytes = data.get(6).copied().map(limit).unwrap_or(1);
    let number_bytes = data.get(7).copied().map(limit).unwrap_or(1);
    let payload_bytes = data.get(8).copied().map(limit).unwrap_or(1);

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
