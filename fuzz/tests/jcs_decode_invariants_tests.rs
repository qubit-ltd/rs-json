// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Regressions for JCS decode limits and input accounting.

use std::io::Cursor;

use qubit_budget::json::JsonDecodeLimits;
use qubit_budget::json::JsonResource;
use qubit_json::decode::JcsDecodeError;
use qubit_json::decode::JcsDecoder;
use qubit_json::decode::JsonDecodeErrorKind;
use serde_json::Value;

#[test]
fn test_jcs_limits_keep_node_limit_when_depth_is_set() {
    let limits = JsonDecodeLimits::<JsonResource, usize>::builder()
        .max_input_bytes(16)
        .max_depth(32)
        .max_nodes(0)
        .build();
    assert_eq!(limits.value_limits().max_nodes(), Some(0));
    assert!(JcsDecoder::with_limits(limits).decode_str("true").is_err());

    let limits = JsonDecodeLimits::<JsonResource, usize>::builder()
        .max_input_bytes(16)
        .max_depth(32)
        .max_nodes(1)
        .build();
    assert!(JcsDecoder::with_limits(limits).decode_str("true").is_ok());

    let limits = JsonDecodeLimits::<JsonResource, usize>::builder()
        .max_input_bytes(16)
        .max_depth(32)
        .max_nodes(1)
        .build();
    assert!(JcsDecoder::with_limits(limits).decode_str("[0]").is_err());

    let limits = JsonDecodeLimits::<JsonResource, usize>::builder()
        .max_input_bytes(16)
        .max_depth(32)
        .max_nodes(2)
        .build();
    assert!(JcsDecoder::with_limits(limits).decode_str("[0]").is_ok());
}

#[test]
fn test_jcs_invariant_harness_checks_zero_and_exact_node_limits() {
    qubit_json_fuzz::jcs_decode_invariants::assert_jcs_decode_invariants(b"true", 0, 32);
    qubit_json_fuzz::jcs_decode_invariants::assert_jcs_decode_invariants(b"true", 1, 32);
    qubit_json_fuzz::jcs_decode_invariants::assert_jcs_decode_invariants(b"[0]", 1, 32);
    qubit_json_fuzz::jcs_decode_invariants::assert_jcs_decode_invariants(b"[0]", 2, 32);
}

#[test]
fn test_jcs_input_limit_accepts_exact_boundary_in_new_session() {
    let limits = JsonDecodeLimits::<JsonResource, usize>::builder()
        .max_input_bytes(4)
        .max_nodes(32)
        .build();
    let mut decoder = JcsDecoder::with_limits(limits);
    let mut input = Cursor::new(b"true".to_vec());

    assert_eq!(decoder.decode_input(&mut input).unwrap(), Value::Bool(true));
    assert_eq!(input.position(), 4);
    assert_eq!(decoder.session().input_budget().unwrap().used(), 4);
    assert_eq!(decoder.session().value_budget().used_nodes(), Some(1));
}

#[test]
fn test_jcs_input_limit_rejects_probe_byte_above_boundary_in_new_session() {
    let limits = JsonDecodeLimits::<JsonResource, usize>::builder()
        .max_input_bytes(4)
        .max_nodes(32)
        .build();
    let mut decoder = JcsDecoder::with_limits(limits);
    let mut input = Cursor::new(b"true ".to_vec());

    let error = decoder
        .decode_input(&mut input)
        .expect_err("the probe byte exceeds the input limit");
    let JcsDecodeError::Decode(error) = error else {
        panic!("input overflow must be a decode budget error")
    };
    assert_eq!(error.kind(), JsonDecodeErrorKind::Budget);
    assert_eq!(input.position(), 5);
    assert_eq!(decoder.session().input_budget().unwrap().used(), 4);
    assert_eq!(decoder.session().value_budget().used_nodes(), Some(0));
}

#[test]
fn test_jcs_input_limit_accumulates_across_decode_attempts() {
    let limits = JsonDecodeLimits::<JsonResource, usize>::builder()
        .max_input_bytes(4)
        .max_nodes(32)
        .build();
    let mut decoder = JcsDecoder::with_limits(limits);
    let mut exact = Cursor::new(b"true".to_vec());
    decoder.decode_input(&mut exact).expect("the first document fits");

    let mut oversized = Cursor::new(b"x".to_vec());
    let error = decoder
        .decode_input(&mut oversized)
        .expect_err("the cumulative input budget is exhausted");
    let JcsDecodeError::Decode(error) = error else {
        panic!("cumulative input overflow must be a decode budget error")
    };
    assert_eq!(error.kind(), JsonDecodeErrorKind::Budget);
    assert_eq!(oversized.position(), 1);
    assert_eq!(decoder.session().input_budget().unwrap().used(), 4);
    assert_eq!(decoder.session().value_budget().used_nodes(), Some(1));
}
