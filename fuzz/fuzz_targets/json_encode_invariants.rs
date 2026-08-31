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
use qubit_json::encode::JsonEncoder;
use serde_json::Value;

mod internal;

const MAX_INPUT_LEN: usize = 4 * 1024;

fuzz_target!(|data: &[u8]| {
    let input = &data[..data.len().min(MAX_INPUT_LEN)];
    let Ok(value) = serde_json::from_slice::<Value>(input) else {
        return;
    };

    let output_bytes = internal::fuzz_limit::limit(data, 0);
    let nodes = internal::fuzz_limit::limit(data, 2);
    let payload_bytes = internal::fuzz_limit::limit(data, 4);
    let structure = StructureLimits::builder().nodes_limit(ResourceLimit::new(JsonResource::Nodes, nodes));
    let value_limits = JsonValueLimits::<JsonResource, usize>::builder()
        .structure_limits(structure)
        .payload_bytes_limit(ResourceLimit::new(JsonResource::PayloadBytes, payload_bytes))
        .build();
    let limits = JsonEncodeLimits::<JsonResource, usize>::builder()
        .output_bytes_limit(ResourceLimit::new(JsonResource::OutputBytes, output_bytes))
        .value_limits(value_limits)
        .build();
    let vector_session = JsonEncodeSession::from_limits(limits);
    let (encoded, vector_session) = {
        let mut encoder = JsonEncoder::new(vector_session);
        let encoded = encoder.to_vec(&value);
        (encoded, encoder.into_session())
    };
    let buffered_session = JsonEncodeSession::from_limits(limits);
    let mut buffered_output = Vec::new();
    let (buffered, buffered_session) = {
        let mut encoder = JsonEncoder::new(buffered_session);
        let buffered = encoder.write_buffered(&mut buffered_output, &value);
        (buffered, encoder.into_session())
    };
    let incremental_session = JsonEncodeSession::from_limits(limits);
    let mut incremental_output = Vec::new();
    let (incremental, incremental_session) = {
        let mut encoder = JsonEncoder::new(incremental_session);
        let incremental = encoder.write_incremental(&mut incremental_output, &value);
        (incremental, encoder.into_session())
    };

    assert!(
        vector_session
            .output_budget()
            .is_none_or(|budget| budget.used() <= output_bytes)
    );
    assert!(vector_session.value_budget().used_nodes() <= Some(nodes));
    assert!(
        buffered_session
            .output_budget()
            .is_none_or(|budget| budget.used() <= output_bytes)
    );
    assert!(buffered_session.value_budget().used_nodes() <= Some(nodes));
    assert!(
        incremental_session
            .output_budget()
            .is_none_or(|budget| budget.used() <= output_bytes)
    );
    assert!(incremental_session.value_budget().used_nodes() <= Some(nodes));
    assert_eq!(encoded.is_ok(), buffered.is_ok());
    assert_eq!(encoded.is_ok(), incremental.is_ok());
    if let Ok(encoded) = encoded {
        let decoded =
            serde_json::from_slice::<Value>(&encoded).expect("successful budget-aware encoding must produce JSON");
        assert_eq!(decoded, value);
        assert_eq!(buffered_output, encoded);
        assert_eq!(incremental_output, encoded);
    }
});
