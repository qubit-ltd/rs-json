// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Verifies JCS decoder input and value budget behavior.

use std::io::Cursor;

use qubit_budget::json::JsonDecodeLimits;
use qubit_budget::json::JsonResource;
use qubit_budget::json::JsonValueLimits;
use qubit_json::decode::JcsDecodeError;
use qubit_json::decode::JcsDecoder;
use qubit_json::decode::JsonDecodeErrorKind;
use serde_json::Value;

#[test]
fn test_jcs_decoder_duplicate_key_rolls_back_values_but_keeps_input() {
    let limits = JsonDecodeLimits::<JsonResource, usize>::builder()
        .max_input_bytes(4096)
        .max_nodes(32)
        .build();
    let mut decoder = JcsDecoder::with_limits(limits);
    let input = r#"{"x":1,"x":2}"#;
    let error = decoder
        .decode_str(input)
        .expect_err("duplicate object keys should be rejected");
    assert!(matches!(error, JcsDecodeError::DuplicateKey { line: 1, column: 8 }));
    assert_eq!(decoder.session().value_budget().used_nodes(), Some(0));
    assert_eq!(
        decoder
            .session()
            .input_budget()
            .expect("input budget is configured")
            .used(),
        input.len()
    );
    decoder
        .decode_str("true")
        .expect("a valid JSON boolean should decode within the remaining limits");
    assert_eq!(decoder.session().value_budget().used_nodes(), Some(1));
    assert_eq!(
        decoder
            .session()
            .input_budget()
            .expect("input budget is configured")
            .used(),
        input.len() + 4
    );
}

#[test]
fn test_jcs_decoder_number_budget_precedes_binary64_range_error() {
    let limits = JsonDecodeLimits::<JsonResource, usize>::builder()
        .value_limits(JsonValueLimits::builder().max_number_bytes(2).build())
        .build();
    let error = JcsDecoder::with_limits(limits)
        .decode_str("1e400")
        .expect_err("number byte limit should reject the oversized token");
    let JcsDecodeError::Decode(error) = error else {
        panic!("expected budget error")
    };
    assert_eq!(error.kind(), JsonDecodeErrorKind::Budget);
}

#[test]
fn test_jcs_decoder_long_integer_obeys_number_bytes_budget() {
    let limits = JsonDecodeLimits::<JsonResource, usize>::builder()
        .value_limits(JsonValueLimits::builder().max_number_bytes(8).build())
        .build();
    let error = JcsDecoder::with_limits(limits)
        .decode_str("100000000000000000000")
        .expect_err("number byte limit should reject the oversized integer token");
    let JcsDecodeError::Decode(error) = error else {
        panic!("expected number token budget error")
    };
    assert_eq!(error.kind(), JsonDecodeErrorKind::Budget);
}

#[test]
fn test_jcs_decoder_depth_budget_rejects_before_value_materialization() {
    let limits = JsonDecodeLimits::<JsonResource, usize>::builder()
        .value_limits(JsonValueLimits::builder().max_depth(2).build())
        .build();
    let mut decoder = JcsDecoder::with_limits(limits);
    let error = decoder
        .decode_str("[[[0]]]")
        .expect_err("a value deeper than the configured limit should fail");
    let JcsDecodeError::Decode(error) = error else {
        panic!("expected depth budget error")
    };
    assert_eq!(error.kind(), JsonDecodeErrorKind::Budget);
    decoder
        .decode_str("[0]")
        .expect("a shallower value remains within the depth limit");
}

#[test]
fn test_jcs_decoder_input_entry_accepts_exact_byte_budget() {
    let limits = JsonDecodeLimits::<JsonResource, usize>::builder()
        .max_input_bytes(4)
        .max_nodes(32)
        .build();
    let mut decoder = JcsDecoder::with_limits(limits);
    let mut exact = Cursor::new(b"true".to_vec());
    assert_eq!(
        decoder
            .decode_input(&mut exact)
            .expect("input exactly at the byte budget should decode"),
        Value::Bool(true)
    );
    assert_eq!(exact.position(), 4);
    assert_eq!(decoder.session().value_budget().used_nodes(), Some(1));
    assert_eq!(
        decoder
            .session()
            .input_budget()
            .expect("input budget is configured")
            .used(),
        4
    );
}

#[test]
fn test_jcs_decoder_input_entry_rejects_overflow_probe_byte() {
    let limits = JsonDecodeLimits::<JsonResource, usize>::builder()
        .max_input_bytes(4)
        .max_nodes(32)
        .build();
    let mut decoder = JcsDecoder::with_limits(limits);
    let mut oversized = Cursor::new(b"true ".to_vec());
    let error = decoder
        .decode_input(&mut oversized)
        .expect_err("the fifth byte must exceed the input budget");
    let JcsDecodeError::Decode(error) = error else {
        panic!("input overflow must be reported as a decode budget error")
    };
    assert_eq!(error.kind(), JsonDecodeErrorKind::Budget);
    assert_eq!(oversized.position(), 5);
    assert_eq!(decoder.session().value_budget().used_nodes(), Some(0));
    assert_eq!(
        decoder
            .session()
            .input_budget()
            .expect("input budget is configured")
            .used(),
        4
    );
}

#[test]
fn test_jcs_decoder_input_entry_keeps_cumulative_budget_exhaustion() {
    let limits = JsonDecodeLimits::<JsonResource, usize>::builder()
        .max_input_bytes(4)
        .max_nodes(32)
        .build();
    let mut decoder = JcsDecoder::with_limits(limits);
    let mut exact = Cursor::new(b"true".to_vec());
    decoder.decode_input(&mut exact).expect("the first input fits");

    let mut next = Cursor::new(b"x".to_vec());
    let error = decoder
        .decode_input(&mut next)
        .expect_err("the shared session has already consumed its input allowance");
    let JcsDecodeError::Decode(error) = error else {
        panic!("cumulative input overflow must be a decode budget error")
    };
    assert_eq!(error.kind(), JsonDecodeErrorKind::Budget);
    assert_eq!(next.position(), 1);
    assert_eq!(decoder.session().value_budget().used_nodes(), Some(1));
    assert_eq!(
        decoder
            .session()
            .input_budget()
            .expect("input budget is configured")
            .used(),
        4
    );
}
