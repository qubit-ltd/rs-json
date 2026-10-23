// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Deterministic regressions for the shared JSON encode invariant harness.

use qubit_json_fuzz::json_encode_invariants::assert_json_encode_invariants;
use qubit_json_fuzz::json_encode_invariants::select_encode_limit;
use qubit_json_fuzz::json_encode_invariants::split_encode_input;

#[test]
fn test_limit_modes_include_zero_and_exact_boundaries() {
    assert_eq!(select_encode_limit(0, 4), 0);
    assert_eq!(select_encode_limit(1, 4), 3);
    assert_eq!(select_encode_limit(2, 4), 4);
    assert_eq!(select_encode_limit(3, 4), 5);
    assert_eq!(select_encode_limit(4, 4), 0);
    assert_eq!(select_encode_limit(20_484, 4), 4_096);
    assert_eq!(select_encode_limit(20_489, 4), 0);
    assert_eq!(select_encode_limit(3, usize::MAX), usize::MAX);
    assert_eq!(select_encode_limit(4, 0), 0);
    assert_eq!(select_encode_limit(5, 0), 0);
    assert_eq!(select_encode_limit(u16::MAX, usize::MAX), 0);
}

#[test]
fn test_split_requires_control_header_and_preserves_complete_payload() {
    for length in [0, 8] {
        assert!(split_encode_input(&vec![0; length]).is_none());
    }

    let mut input = vec![0; 9];
    assert_eq!(split_encode_input(&input), Some((&[0; 9], &[][..])));
    input.extend_from_slice(b"null");
    let (control, payload) = split_encode_input(&input).expect("complete input must split");
    assert_eq!(control, &[0; 9]);
    assert_eq!(payload, b"null");

    assert!(split_encode_input(&vec![0; 4097]).is_none());
    assert!(split_encode_input(&vec![0; 4096]).is_some());
}

#[test]
fn test_shared_harness_covers_payload_and_writer_invariants() {
    let payloads: &[&[u8]] = &[
        b"null",
        "\"é\\n\\\"\"".as_bytes(),
        br#"{"nested":[true,17,{"key":"value"}]}"#,
        b"\"\"",
    ];

    for payload in payloads {
        for mode in 0_u8..3 {
            for prefix in [0_u8, 1, 255] {
                for max_chunk in [0_u8, 15] {
                    let mut input = vec![2, 0, 2, 0, 2, 0, mode, prefix, max_chunk];
                    input.extend_from_slice(payload);
                    assert_json_encode_invariants(&input);
                }
            }
        }

        for resource in 0..3 {
            for selector in 0_u8..=3 {
                let mut input = vec![2, 0, 2, 0, 2, 0, 0, 0, 0];
                input[resource * 2] = selector;
                input.extend_from_slice(payload);
                assert_json_encode_invariants(&input);
            }
        }
    }

    for prefix in [0_u8, 1, 4] {
        let mut input = vec![2, 0, 2, 0, 2, 0, 2, prefix, 0];
        input.extend_from_slice(b"null");
        assert_json_encode_invariants(&input);
    }
}
