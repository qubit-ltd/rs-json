// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
// =============================================================================
//! Tests the shared fuzz-input boundary.

use qubit_json_fuzz::input_limit::MAX_FUZZ_INPUT_BYTES;
use qubit_json_fuzz::input_limit::bounded_input;

/// Verifies inputs at the configured fuzz boundary remain fully observable.
#[test]
fn test_bounded_input_accepts_the_complete_configured_boundary() {
    let input = vec![0_u8; MAX_FUZZ_INPUT_BYTES];

    assert_eq!(bounded_input(&input), Some(input.as_slice()));
}

/// Verifies inputs above the configured boundary are rejected rather than
/// silently truncated to a common prefix.
#[test]
fn test_bounded_input_rejects_inputs_above_the_configured_boundary() {
    let input = vec![0_u8; MAX_FUZZ_INPUT_BYTES + 1];

    assert_eq!(bounded_input(&input), None);
}
