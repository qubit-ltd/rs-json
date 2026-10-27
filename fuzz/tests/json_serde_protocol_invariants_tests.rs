// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Deterministic coverage for private serde_json protocol fuzz cases.

use std::cell::Cell;

use qubit_json::encode::JsonSerializationErrorKind;
use qubit_json::encode::JsonSerializerStateError;
use qubit_json::value::JsonValueEncoder;
use qubit_json_fuzz::json_serde_protocol_invariants::ProtocolCase;
use qubit_json_fuzz::json_serde_protocol_invariants::assert_json_integer_boundary_invariants;
use qubit_json_fuzz::json_serde_protocol_invariants::assert_json_serde_protocol_invariants;

fn run(
    control: [u8; 9],
    payload: &'static str,
) -> (
    Result<serde_json::Value, qubit_json::encode::JsonSerializationError>,
    usize,
    usize,
) {
    let field_attempts = Cell::new(0);
    let payload_callbacks = Cell::new(0);
    let case = ProtocolCase::new(&control, payload, &field_attempts, &payload_callbacks);
    let result = JsonValueEncoder::new().encode(&case);
    (result, field_attempts.get(), payload_callbacks.get())
}

#[test]
fn test_private_protocol_success_and_payload_oracles() {
    for (control, payload, expected) in [
        ([0, 0, 1, 1, 0, 0, 0, 0, 0], "null", serde_json::Value::Null),
        ([1, 0, 1, 1, 0, 0, 2, 0, 0], "1.25", serde_json::json!(1.25)),
        ([1, 0, 1, 1, 0, 0, 2, 1, 1], "1.25", serde_json::json!(1.25)),
    ] {
        let (result, field_attempts, payload_callbacks) = run(control, payload);
        assert_eq!(field_attempts, 1, "the private field must be attempted");
        assert_eq!(payload_callbacks, 1, "the payload Serialize callback must run");
        assert_eq!(
            result.expect("valid protocol payload").to_string(),
            expected.to_string()
        );
    }
}

#[test]
fn test_private_protocol_failures_are_classified_after_callback() {
    let cases = [
        // Missing field, duplicated field, wrong field name, and non-string payload.
        (
            [0, 0, 1, 0, 0, 0, 0, 0, 0],
            "null",
            JsonSerializationErrorKind::InvalidSerializerState {
                reason: JsonSerializerStateError::InvalidRawValueProtocol,
            },
            0,
            0,
        ),
        (
            [0, 0, 1, 2, 0, 0, 0, 0, 0],
            "null",
            JsonSerializationErrorKind::InvalidSerializerState {
                reason: JsonSerializerStateError::InvalidRawValueProtocol,
            },
            2,
            1,
        ),
        (
            [0, 1, 1, 1, 0, 0, 0, 0, 0],
            "null",
            JsonSerializationErrorKind::InvalidSerializerState {
                reason: JsonSerializerStateError::InvalidRawValueProtocol,
            },
            1,
            0,
        ),
        (
            [1, 0, 1, 1, 0, 1, 2, 0, 0],
            "1.25",
            JsonSerializationErrorKind::InvalidSerializerState {
                reason: JsonSerializerStateError::InvalidNumberProtocol,
            },
            1,
            1,
        ),
        // Exact field but invalid lexical payloads.
        (
            [0, 0, 1, 1, 0, 0, 2, 0, 0],
            "[",
            JsonSerializationErrorKind::InvalidRawValue,
            1,
            1,
        ),
        (
            [1, 0, 1, 1, 0, 0, 5, 0, 0],
            "01",
            JsonSerializationErrorKind::InvalidNumberRepresentation,
            1,
            1,
        ),
        (
            [1, 0, 1, 1, 0, 0, 2, 3, 8],
            "1.25",
            JsonSerializationErrorKind::DisplayFormattingFailed,
            1,
            1,
        ),
    ];
    for (control, payload, expected_error, expected_fields, expected_payload_callbacks) in cases {
        let (result, fields, payload_callbacks) = run(control, payload);
        assert_eq!(
            fields, expected_fields,
            "case must attempt the intended field callbacks"
        );
        assert_eq!(
            payload_callbacks, expected_payload_callbacks,
            "case must reach the intended payload Serialize callbacks"
        );
        assert_eq!(result.expect_err("malformed protocol must fail").kind(), expected_error);
    }
}

#[test]
fn test_i128_and_u128_boundary_matrix_matches_public_range_oracle() {
    for selector in 0_u8..10 {
        assert_json_integer_boundary_invariants(selector);
    }
}

#[test]
fn test_sticky_protocol_failure_skips_later_payload_serialize_callback() {
    let control = [0, 0, 1, 2, 0, 1, 0, 0, 0];
    let (result, field_attempts, payload_callbacks) = run(control, "null");
    assert_eq!(field_attempts, 2, "both fields must be submitted to the compound");
    assert_eq!(
        payload_callbacks, 1,
        "the first payload fails; sticky state must prevent the second payload callback"
    );
    assert_eq!(
        result.expect_err("first invalid field poisons the protocol").kind(),
        JsonSerializationErrorKind::InvalidSerializerState {
            reason: JsonSerializerStateError::InvalidRawValueProtocol,
        }
    );
}

#[test]
fn test_protocol_display_chunk_and_failure_position_matrix() {
    for chunks in 1_u8..=8 {
        for fail_selector in 0_u8..=8 {
            let mut control = [1, 0, 1, 1, 0, 0, 2, 1, 0];
            control[8] = (chunks - 1) + 8 * fail_selector;
            let mut input = control.to_vec();
            input.extend_from_slice(b"1.25");
            assert_json_serde_protocol_invariants(&input);
        }
    }
}

#[test]
fn test_protocol_fuzz_harness_covers_fixed_protocol_matrix() {
    for protocol in 0_u8..=1 {
        for field in 0_u8..=3 {
            for count in 0_u8..=2 {
                for operations in 0_u8..=8 {
                    for payload in 0_u8..=6 {
                        for payload_kind in 0_u8..=3 {
                            let mut input = vec![0; 9];
                            input[0] = protocol;
                            input[1] = field;
                            input[2] = count;
                            input[3] = operations;
                            input[5] = payload_kind;
                            input[6] = payload;
                            assert_json_serde_protocol_invariants(&input);
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn test_budget_selectors_cover_zero_exact_and_one_over_for_each_resource() {
    for payload_selector in [0_u8, 1] {
        for selector in [0_u8, 2, 3] {
            let input = vec![0, 0, 1, 1, 0, 0, payload_selector, 0, 255, selector, selector, selector];
            assert_json_serde_protocol_invariants(&input);
        }
    }
}

#[test]
fn test_incremental_writer_failure_precedes_invalid_protocol_at_zero_prefix() {
    let seed = [2, 0, 88, 26, 0, 0, 37, 2, 0];
    assert_json_serde_protocol_invariants(&seed);
}

#[test]
fn test_first_display_failure_remains_sticky_before_later_protocol_errors() {
    let seed = [2, 0, 100, 222, 68, 164, 207, 219, 93];
    assert_json_serde_protocol_invariants(&seed);
}

#[test]
fn test_buffered_writer_failure_charges_accepted_prefix_and_rolls_back_value() {
    let seed = [2, 0, 1, 100, 115, 40, 89, 89, 2];
    assert_json_serde_protocol_invariants(&seed);
}
