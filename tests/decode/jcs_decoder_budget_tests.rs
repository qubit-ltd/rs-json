// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use qubit_budget::json::JsonDecodeLimits;
use qubit_budget::json::JsonResource;
use qubit_json::decode::JcsDecoder;

#[test]
fn test_jcs_decoder_duplicate_key_rolls_back_values_but_keeps_input() {
    let limits = JsonDecodeLimits::<JsonResource, usize>::builder()
        .max_input_bytes(4096)
        .max_nodes(32)
        .build();
    let mut decoder = JcsDecoder::with_limits(limits);
    let input = r#"{"x":1,"x":2}"#;
    assert!(decoder.decode_str(input).is_err());
    assert_eq!(decoder.session().value_budget().used_nodes(), Some(0));
    assert_eq!(decoder.session().input_budget().unwrap().used(), input.len());
    decoder.decode_str("true").unwrap();
    assert_eq!(decoder.session().value_budget().used_nodes(), Some(1));
    assert_eq!(decoder.session().input_budget().unwrap().used(), input.len() + 4);
}

#[test]
fn test_jcs_decoder_number_budget_precedes_binary64_range_error() {
    let limits = JsonDecodeLimits::<JsonResource, usize>::builder()
        .value_limits(
            qubit_budget::json::JsonValueLimits::builder()
                .max_number_bytes(2)
                .build(),
        )
        .build();
    let error = JcsDecoder::with_limits(limits).decode_str("1e400").unwrap_err();
    let qubit_json::decode::JcsDecodeError::Decode(error) = error else {
        panic!("expected budget error")
    };
    assert_eq!(error.kind(), qubit_json::decode::JsonDecodeErrorKind::Budget);
}

#[test]
fn test_jcs_decoder_long_integer_obeys_number_bytes_budget() {
    let limits = JsonDecodeLimits::<JsonResource, usize>::builder()
        .value_limits(
            qubit_budget::json::JsonValueLimits::builder()
                .max_number_bytes(8)
                .build(),
        )
        .build();
    let error = JcsDecoder::with_limits(limits)
        .decode_str("100000000000000000000")
        .unwrap_err();
    let qubit_json::decode::JcsDecodeError::Decode(error) = error else {
        panic!("expected number token budget error")
    };
    assert_eq!(error.kind(), qubit_json::decode::JsonDecodeErrorKind::Budget);
}

#[test]
fn test_jcs_decoder_depth_budget_rejects_before_value_materialization() {
    let limits = JsonDecodeLimits::<JsonResource, usize>::builder()
        .value_limits(qubit_budget::json::JsonValueLimits::builder().max_depth(2).build())
        .build();
    let mut decoder = JcsDecoder::with_limits(limits);
    let error = decoder.decode_str("[[[0]]]").unwrap_err();
    let qubit_json::decode::JcsDecodeError::Decode(error) = error else {
        panic!("expected depth budget error")
    };
    assert_eq!(error.kind(), qubit_json::decode::JsonDecodeErrorKind::Budget);
    decoder
        .decode_str("[0]")
        .expect("a shallower value remains within the depth limit");
}

#[test]
fn test_jcs_decoder_input_entry_respects_exact_byte_budget() {
    let limits = JsonDecodeLimits::<JsonResource, usize>::builder()
        .max_input_bytes(4)
        .build();
    let mut decoder = JcsDecoder::with_limits(limits);
    let mut exact = std::io::Cursor::new(b"true".to_vec());
    assert_eq!(decoder.decode_input(&mut exact).unwrap(), serde_json::Value::Bool(true));
    let mut oversized = std::io::Cursor::new(b"false".to_vec());
    assert!(decoder.decode_input(&mut oversized).is_err());
    assert_eq!(decoder.session().input_budget().unwrap().used(), 4);
}
