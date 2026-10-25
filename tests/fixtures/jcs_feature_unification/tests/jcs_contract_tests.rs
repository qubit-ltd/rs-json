// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

#![cfg(feature = "jcs")]

use qubit_budget::json::JsonDecodeLimits;
use qubit_budget::json::JsonResource;
use qubit_budget::json::JsonValueLimits;
use qubit_json::decode::JcsDecodeError;
use qubit_json::decode::JcsDecoder;
use qubit_json::encode::JcsEncoder;
use serde_json::Value;

#[test]
fn test_number_contract_and_all_entry_points_are_stable() {
    for input in [
        "1.5",
        "9007199254740993",
        "9007199254740993.0",
        "9.007199254740993e15",
        "1e20",
        "-0",
        "5e-324",
        "1e-400",
    ] {
        let from_str = JcsDecoder::unlimited().decode_str(input).unwrap();
        let from_utf8 = JcsDecoder::unlimited().decode_utf8(input.as_bytes()).unwrap();
        let mut cursor = std::io::Cursor::new(input.as_bytes());
        let from_input = JcsDecoder::unlimited().decode_input(&mut cursor).unwrap();
        assert_eq!(from_str, from_utf8, "{input}");
        assert_eq!(from_str, from_input, "{input}");
        assert_eq!(
            from_str.as_number().unwrap().as_f64().unwrap().to_bits(),
            from_utf8.as_number().unwrap().as_f64().unwrap().to_bits(),
        );
        assert_eq!(
            from_str.as_number().unwrap().as_f64().unwrap().to_bits(),
            from_input.as_number().unwrap().as_f64().unwrap().to_bits(),
        );
    }
    assert!(JcsDecoder::unlimited().decode_str("1e400").is_err());
}

#[test]
fn test_object_marker_nesting_duplicate_keys_and_canonical_bytes() {
    let marker = r#"{"$serde_json::private::Number":"12345678901234567890"}"#;
    assert!(JcsDecoder::unlimited().decode_str(marker).unwrap().is_object());
    assert!(JcsDecoder::unlimited()
        .decode_str(r#"{"a":{"$serde_json::private::Number":"2"},"s":"ordinary"}"#)
        .unwrap()
        .is_object());
    assert!(matches!(
        JcsDecoder::unlimited().decode_str(r#"{"a":{"x":1,"x":2}}"#),
        Err(JcsDecodeError::DuplicateKey { .. })
    ));
    let value = JcsDecoder::unlimited().decode_str(r#"{"😀":1,"":2,"é":"ok"}"#).unwrap();
    assert_eq!(
        JcsEncoder::unlimited().to_vec(&value).unwrap(),
        r#"{"é":"ok","😀":1,"":2}"#.as_bytes(),
    );
    assert_eq!(
        JcsEncoder::unlimited()
            .to_vec(&JcsDecoder::unlimited().decode_str(r#"{"b":2,"a":1}"#).unwrap())
            .unwrap(),
        br#"{"a":1,"b":2}"#,
    );
}

#[test]
fn test_budget_precedence_rollback_and_decoder_recovery() {
    let limits = JsonDecodeLimits::<JsonResource, usize>::builder()
        .max_input_bytes(9)
        .max_nodes(8)
        .value_limits(JsonValueLimits::builder().max_number_bytes(2).max_nodes(8).build())
        .build();
    let mut decoder = JcsDecoder::with_limits(limits);
    let error = decoder.decode_str("1e400").unwrap_err();
    let JcsDecodeError::Decode(error) = error else {
        panic!("expected budget error")
    };
    assert_eq!(error.kind(), qubit_json::decode::JsonDecodeErrorKind::Budget);
    assert_eq!(decoder.session().value_budget().used_nodes(), Some(0));
    assert_eq!(decoder.session().input_budget().unwrap().used(), 5);
    assert_eq!(decoder.decode_str("true").unwrap(), Value::Bool(true));
    assert_eq!(decoder.session().value_budget().used_nodes(), Some(1));
    assert_eq!(decoder.session().input_budget().unwrap().used(), 9);
}
