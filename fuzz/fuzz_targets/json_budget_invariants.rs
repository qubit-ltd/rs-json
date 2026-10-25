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
use qubit_budget::ResourceLimit;
use qubit_budget::StructureLimits;
use qubit_budget::json::JsonResource;
use qubit_budget::json::JsonValueLimits;
use qubit_json_fuzz::input_limit::bounded_input;
use qubit_json_fuzz::json_budget_invariants::assert_json_budget_invariants;

mod internal;

fuzz_target!(|data: &[u8]| {
    let Some(input) = bounded_input(data) else {
        return;
    };
    let bytes = internal::fuzz_limit::limit(data, 0);
    let depth = internal::fuzz_limit::limit(data, 2);
    let nodes = internal::fuzz_limit::limit(data, 4);
    let items = internal::fuzz_limit::limit(data, 6);
    let entries = internal::fuzz_limit::limit(data, 8);
    let key_bytes = internal::fuzz_limit::limit(data, 10);
    let string_bytes = internal::fuzz_limit::limit(data, 12);
    let number_bytes = internal::fuzz_limit::limit(data, 14);
    let payload_bytes = internal::fuzz_limit::limit(data, 16);

    let structure = StructureLimits::builder()
        .depth_limit(ResourceLimit::new(JsonResource::Depth, depth))
        .nodes_limit(ResourceLimit::new(JsonResource::Nodes, nodes))
        .sequence_items_limit(ResourceLimit::new(JsonResource::SequenceItems, items))
        .map_entries_limit(ResourceLimit::new(JsonResource::MapEntries, entries))
        .key_bytes_limit(ResourceLimit::new(JsonResource::KeyBytes, key_bytes));
    let value_limits = JsonValueLimits::<JsonResource, usize>::builder()
        .structure_limits(structure)
        .string_bytes_limit(ResourceLimit::new(JsonResource::StringBytes, string_bytes))
        .number_bytes_limit(ResourceLimit::new(JsonResource::NumberBytes, number_bytes))
        .payload_bytes_limit(ResourceLimit::new(JsonResource::PayloadBytes, payload_bytes))
        .build();
    assert_json_budget_invariants(input, bytes, value_limits);
});
