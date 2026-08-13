// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Fuzzes JSON encoding output and accounting invariants.

#![no_main]

use libfuzzer_sys::fuzz_target;
use qubit_budget::ResourceLimit;
use qubit_budget::StructureLimits;
use qubit_budget::json::JsonEncodeLimits;
use qubit_budget::json::JsonEncodeSession;
use qubit_budget::json::JsonResource;
use qubit_budget::json::JsonValueLimits;
use qubit_json::text::encode_to_vec;
use serde_json::Value;

const MAX_INPUT_LEN: usize = 4 * 1024;
const MAX_LIMIT: usize = 4 * 1024;

fn limit(data: u8) -> usize {
    1 + usize::from(data) % MAX_LIMIT
}

fuzz_target!(|data: &[u8]| {
    let input = &data[..data.len().min(MAX_INPUT_LEN)];
    let Ok(value) = serde_json::from_slice::<Value>(input) else {
        return;
    };

    let output_bytes = data.first().copied().map(limit).unwrap_or(1);
    let nodes = data.get(1).copied().map(limit).unwrap_or(1);
    let payload_bytes = data.get(2).copied().map(limit).unwrap_or(1);
    let structure = StructureLimits::empty()
        .with_nodes_limit(ResourceLimit::new(JsonResource::Nodes, nodes));
    let value_limits = JsonValueLimits::empty()
        .with_structure_limits(structure)
        .with_payload_bytes_limit(ResourceLimit::new(
            JsonResource::PayloadBytes,
            payload_bytes,
        ));
    let limits = JsonEncodeLimits::empty()
        .with_output_bytes_limit(ResourceLimit::new(
            JsonResource::OutputBytes,
            output_bytes,
        ))
        .with_value_limits(value_limits);
    let mut session = JsonEncodeSession::owned(limits);
    let encoded = encode_to_vec(&value, &mut session);

    assert!(
        session
            .output_budget()
            .is_none_or(|budget| budget.used() <= output_bytes)
    );
    assert!(session.value_budget().structure_budget().used_nodes() <= nodes);
    if let Ok(encoded) = encoded {
        let decoded = serde_json::from_slice::<Value>(&encoded)
            .expect("successful budget-aware encoding must produce JSON");
        assert_eq!(decoded, value);
    }
});
