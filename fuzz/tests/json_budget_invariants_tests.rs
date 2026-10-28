// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Deterministic coverage for JSON fuzz budget transaction assertions.

use qubit_budget::json::JsonDecodeLimits;
use qubit_budget::json::JsonResource;
use qubit_budget::json::JsonValueLimits;
use qubit_json::decode::JsonDecodeErrorKind;
use qubit_json::decode::JsonDecoder;
use qubit_json_fuzz::json_budget_invariants::CONTROL_BYTES;
use qubit_json_fuzz::json_budget_invariants::assert_json_budget_fuzz_input;
use qubit_json_fuzz::json_budget_invariants::assert_json_budget_invariants;
use qubit_json_fuzz::json_budget_invariants::select_budget_limit;
use qubit_json_fuzz::json_budget_invariants::split_budget_input;
use serde::de::IgnoredAny;

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

#[test]
fn test_budget_control_header_split_and_complete_input_boundary() {
    assert!(split_budget_input(&[]).is_none());
    assert!(split_budget_input(&[0; 17]).is_none());
    let header = [0; CONTROL_BYTES];
    let (control, payload) = split_budget_input(&header).expect("header-only input is valid");
    assert_eq!(control, &header);
    assert!(payload.is_empty());

    let mut max_input = vec![0; 4096];
    max_input[..CONTROL_BYTES].fill(7);
    let (control, payload) = split_budget_input(&max_input).expect("4096 bytes are accepted");
    assert_eq!(control, &[7; CONTROL_BYTES]);
    assert_eq!(payload.len(), 4096 - CONTROL_BYTES);
    max_input.push(0);
    assert!(split_budget_input(&max_input).is_none());
}

#[test]
fn test_budget_limit_selector_covers_all_modes_and_zero_baselines() {
    for (raw, expected) in [(0, 0), (1, 0), (2, 0), (3, 1), (4, 0)] {
        assert_eq!(select_budget_limit(raw, 0), expected);
    }
    for (raw, expected) in [(0, 0), (1, 8), (2, 9), (3, 10), (4, 0)] {
        assert_eq!(select_budget_limit(raw, 9), expected);
    }
}

#[test]
fn test_budget_fuzz_entry_exercises_exact_and_each_independent_boundary() {
    let payload = r#"["é",1e2,{"k":"\\n","k":2,"$serde_json::private::RawValue":"x"}]"#.as_bytes();
    let mut exact = [2_u16.to_le_bytes(); 9].concat();
    exact.extend_from_slice(payload);
    assert_json_budget_fuzz_input(&exact);

    for dimension in 0..9 {
        let first_byte = (0_u16..=255)
            .map(|candidate| candidate as u8)
            .find(|candidate| usize::from(*candidate) % 9 == dimension)
            .expect("each dimension has a control byte selecting below its exact use");
        let mut input = [2_u16.to_le_bytes(); 9].concat();
        input[0] = first_byte;
        input[1] = 0;
        if dimension > 0 {
            input[dimension * 2] = 1;
            input[dimension * 2 + 1] = 0;
        }
        input.extend_from_slice(payload);
        assert_json_budget_fuzz_input(&input);
    }
}

#[test]
fn test_budget_fuzz_entry_keeps_invalid_and_noncanonical_payload_paths() {
    let mut input = [2_u16.to_le_bytes(); 9].concat();
    for payload in [
        br#"{"x":]"#.as_slice(),
        b"18446744073709551616",
        b"1e2",
        br#"{"$serde_json::private::RawValue":"[1,2]"}"#,
    ] {
        input.truncate(CONTROL_BYTES);
        input.extend_from_slice(payload);
        assert_json_budget_fuzz_input(&input);
    }
}

#[test]
fn test_budget_reference_boundaries_match_each_resource() {
    let input = br#"["x",{"k":2}]"#;
    let exact = [13, 3, 4, 2, 1, 1, 1, 1, 3];
    for dimension in 0..9 {
        let mut exact_limits = exact;
        assert!(decode_with_limits(input, exact_limits).is_ok());

        exact_limits[dimension] += 1;
        assert!(decode_with_limits(input, exact_limits).is_ok());

        let mut below = exact;
        below[dimension] -= 1;
        let error = decode_with_limits(input, below).expect_err("one short resource limit rejects");
        assert_eq!(error.kind(), JsonDecodeErrorKind::Budget);
        let budget_error = error
            .budget_error()
            .and_then(|error| error.budget_error())
            .expect("budget rejection identifies its resource");
        assert_eq!(budget_error.resource(), resource_for(dimension));
    }
}

#[test]
fn test_budget_zero_usage_dimensions_accept_zero_limits() {
    let input = b"null";
    let exact = [4, 1, 1, 0, 0, 0, 0, 0, 0];
    for dimension in 3..9 {
        let mut limits = exact;
        limits[dimension] = 0;
        assert!(decode_with_limits(input, limits).is_ok());
    }
}

#[test]
fn test_budget_cumulative_input_node_and_payload_accounting() {
    let input = br#"["x"]"#;
    let limits = JsonDecodeLimits::<JsonResource, usize>::builder()
        .max_input_bytes(input.len() * 2)
        .max_nodes(3)
        .max_payload_bytes(1)
        .build();
    let mut decoder = JsonDecoder::with_limits(limits);
    decoder.decode_utf8::<IgnoredAny>(input).expect("first value fits");
    let error = decoder
        .decode_utf8::<IgnoredAny>(input)
        .expect_err("cumulative nodes reject the second value");
    assert_eq!(error.kind(), JsonDecodeErrorKind::Budget);
    assert_eq!(
        error
            .budget_error()
            .and_then(|error| error.budget_error())
            .expect("node limit identifies a budget resource")
            .resource(),
        &JsonResource::Nodes
    );
    assert_eq!(decoder.session().value_budget().used_nodes(), Some(2));
    assert_eq!(decoder.session().value_budget().used_payload_bytes(), Some(1));
    assert_eq!(decoder.session().input_budget().unwrap().used(), input.len() * 2);

    let limits = JsonDecodeLimits::<JsonResource, usize>::builder()
        .max_input_bytes(input.len())
        .max_nodes(8)
        .max_payload_bytes(8)
        .build();
    let mut decoder = JsonDecoder::with_limits(limits);
    decoder.decode_utf8::<IgnoredAny>(input).expect("first value fits");
    let error = decoder
        .decode_utf8::<IgnoredAny>(input)
        .expect_err("cumulative input rejects the second value");
    assert_eq!(error.kind(), JsonDecodeErrorKind::Budget);
    assert_eq!(decoder.session().input_budget().unwrap().used(), input.len());
    assert_eq!(decoder.session().value_budget().used_nodes(), Some(2));
    assert_eq!(decoder.session().value_budget().used_payload_bytes(), Some(1));
}

fn decode_with_limits(
    input: &[u8],
    limits: [usize; 9],
) -> Result<IgnoredAny, qubit_json::decode::JsonDecodeError<JsonResource, usize>> {
    let limits = JsonDecodeLimits::<JsonResource, usize>::builder()
        .max_input_bytes(limits[0])
        .max_depth(limits[1])
        .max_nodes(limits[2])
        .max_sequence_items(limits[3])
        .max_map_entries(limits[4])
        .max_key_bytes(limits[5])
        .max_string_bytes(limits[6])
        .max_number_bytes(limits[7])
        .max_payload_bytes(limits[8])
        .build();
    JsonDecoder::with_limits(limits).decode_utf8::<IgnoredAny>(input)
}

fn resource_for(dimension: usize) -> &'static JsonResource {
    match dimension {
        0 => &JsonResource::InputBytes,
        1 => &JsonResource::Depth,
        2 => &JsonResource::Nodes,
        3 => &JsonResource::SequenceItems,
        4 => &JsonResource::MapEntries,
        5 => &JsonResource::KeyBytes,
        6 => &JsonResource::StringBytes,
        7 => &JsonResource::NumberBytes,
        8 => &JsonResource::PayloadBytes,
        _ => unreachable!(),
    }
}
