// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Deterministic coverage for JSON fuzz budget transaction assertions.

use qubit_budget::json::JsonResource;
use qubit_budget::json::JsonValueLimits;
use qubit_json_fuzz::json_budget_invariants::assert_json_budget_invariants;

#[test]
fn test_budget_harness_checks_success_failure_and_seed_commit_paths() {
    for input in [
        b"null".as_slice(),
        br#"{"key":[1,"value"]}"#,
        "\"é中\"".as_bytes(),
        br#"{"key":]"#,
        b"null true",
        br#"{"$serde_json::private::RawValue":"[1,2,3]"}"#,
    ] {
        for (nodes, payload) in [(64, 256), (1, 256), (64, 1), (0, 0)] {
            let limits = JsonValueLimits::<JsonResource, usize>::builder()
                .max_nodes(nodes)
                .max_payload_bytes(payload)
                .build();
            assert_json_budget_invariants(input, input.len() * 2, limits);
        }
    }
}

#[test]
fn test_budget_harness_handles_cumulative_exhaustion_and_optional_counters() {
    for (nodes, payload) in [(3, 100), (100, 1), (100, 100)] {
        let limits = JsonValueLimits::<JsonResource, usize>::builder()
            .max_nodes(nodes)
            .max_payload_bytes(payload)
            .build();
        assert_json_budget_invariants(br#"["x"]"#, 64, limits);
    }
    let limits = JsonValueLimits::<JsonResource, usize>::builder().max_depth(4).build();
    assert_json_budget_invariants(b"null", 4, limits);
    let limits = JsonValueLimits::<JsonResource, usize>::builder()
        .max_nodes(64)
        .max_payload_bytes(256)
        .build();
    assert_json_budget_invariants(br#"{"key":1}"#, 1, limits);
}
