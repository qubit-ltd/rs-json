// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Deterministic checks for the mutable JSON tree fuzz harness.

use qubit_json_fuzz::json_tree_invariants::assert_json_tree_invariants;

#[test]
fn test_tree_invariants_accepts_descending_and_skipping_shapes() {
    for input in [
        b"\0null".as_slice(),
        b"\0[null,[],{},[1,2]]".as_slice(),
        "\0{\"secret\":\"root\",\"兄弟\":{\"secret\":\"child\",\"keep\":[1,2]},\"$serde_json::private::RawValue\":\"x\",\"z\":7}".as_bytes(),
        b"\x01{\"secret\":\"root\",\"nested\":{\"secret\":\"child\",\"items\":[1,2]}}".as_slice(),
    ] {
        assert_json_tree_invariants(input);
    }
}

#[test]
fn test_tree_invariants_accepts_input_with_control_header_only() {
    assert_json_tree_invariants(&[0]);
    assert_json_tree_invariants(&[1]);
}

#[test]
fn test_tree_invariants_preserves_insertion_order_across_siblings() {
    let mut input = vec![0_u8];
    input.extend_from_slice(br#"{"z":{"secret":"a","keep":1},"a":{"secret":"b","keep":2},"secret":"root"}"#);
    assert_json_tree_invariants(&input);
}
