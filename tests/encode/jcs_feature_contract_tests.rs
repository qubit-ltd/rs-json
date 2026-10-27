// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

#![cfg(feature = "jcs")]

use qubit_budget::json::JsonDecodeLimits;
use qubit_budget::json::JsonEncodeLimits;
use qubit_budget::json::JsonResource;
use qubit_budget::json::JsonValueLimits;
use qubit_json::decode::JcsDecodeError;
use qubit_json::decode::JcsDecoder;
use qubit_json::encode::JcsEncodeErrorKind;
use qubit_json::encode::JcsEncoder;
use qubit_json::encode::JsonSerializationErrorCategory;
use qubit_json::encode::JsonSerializationErrorKind;
use serde_json::Value;

#[test]
fn test_jcs_numeric_overflow_has_structured_reason() {
    if !supports_arbitrary_precision() {
        assert!(serde_json::from_str::<Value>("1e400").is_err());
        assert!(serde_json::from_str::<Value>("-1e400").is_err());
        return;
    }
    for text in ["1e400", "-1e400"] {
        let value: Value = serde_json::from_str(text).unwrap();
        let error = JcsEncoder::unlimited().to_vec(&value).unwrap_err();
        assert_eq!(error.kind(), JcsEncodeErrorKind::Serialize);
        let source = error.serialization_error().unwrap();
        assert_eq!(source.kind(), JsonSerializationErrorKind::NonFiniteFloat);
        assert_eq!(source.category(), JsonSerializationErrorCategory::Number);
        assert!(!format!("{error:?} {error}").contains(text));
    }
}

#[test]
fn test_jcs_numeric_failure_retains_output_boundaries() {
    if !supports_arbitrary_precision() {
        assert!(serde_json::from_str::<Value>("[0,1e400]").is_err());
        return;
    }
    let value: Value = serde_json::from_str("[0,1e400]").unwrap();
    for mode in 0..4 {
        let limits = JsonEncodeLimits::<JsonResource, usize>::builder()
            .max_nodes(8)
            .max_output_bytes(64)
            .build();
        let mut encoder = JcsEncoder::with_limits(limits, 16 * 1024);
        let mut bytes = Vec::new();
        let error = match mode {
            0 => encoder.to_vec(&value).unwrap_err(),
            1 => encoder.to_string(&value).unwrap_err(),
            2 => encoder.write_buffered(&mut bytes, &value).unwrap_err(),
            _ => encoder.write_incremental(&mut bytes, &value).unwrap_err(),
        };
        assert_eq!(error.kind(), JcsEncodeErrorKind::Serialize);
        assert_eq!(
            error.serialization_error().unwrap().kind(),
            JsonSerializationErrorKind::NonFiniteFloat,
        );
        let expected: &[u8] = if mode == 3 { b"[0," } else { b"" };
        assert_eq!(bytes, expected);
        assert_eq!(encoder.session().output_budget().unwrap().used(), expected.len());
        assert_eq!(encoder.session().value_budget().used_nodes(), Some(0));
    }
}

#[test]
fn test_jcs_numeric_failure_respects_value_and_output_budgets() {
    if !supports_arbitrary_precision() {
        assert!(serde_json::from_str::<Value>("[0,1e400]").is_err());
        return;
    }
    let value: Value = serde_json::from_str("[0,1e400]").unwrap();

    let limits = JsonEncodeLimits::<JsonResource, usize>::builder().max_nodes(1).build();
    let mut encoder = JcsEncoder::with_limits(limits, 16 * 1024);
    let mut bytes = Vec::new();
    let error = encoder.write_incremental(&mut bytes, &value).unwrap_err();
    assert_eq!(error.kind(), JcsEncodeErrorKind::Budget);
    assert!(bytes.is_empty());
    assert_eq!(encoder.session().value_budget().used_nodes(), Some(0));

    let limits = JsonEncodeLimits::<JsonResource, usize>::builder()
        .max_nodes(8)
        .max_output_bytes(0)
        .build();
    let mut encoder = JcsEncoder::with_limits(limits, 16 * 1024);
    let mut bytes = Vec::new();
    let error = encoder.write_incremental(&mut bytes, &value).unwrap_err();
    assert_eq!(error.kind(), JcsEncodeErrorKind::Budget);
    assert!(bytes.is_empty());
    assert_eq!(encoder.session().value_budget().used_nodes(), Some(0));
    assert_eq!(encoder.session().output_budget().unwrap().used(), 0);
}

#[test]
fn test_jcs_numeric_failure_does_not_override_writer_failure() {
    if !supports_arbitrary_precision() {
        assert!(serde_json::from_str::<Value>("[0,1e400]").is_err());
        return;
    }
    struct RejectingWriter;

    impl std::io::Write for RejectingWriter {
        fn write(&mut self, _buffer: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("injected destination failure"))
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    let value: Value = serde_json::from_str("[0,1e400]").unwrap();
    let limits = JsonEncodeLimits::<JsonResource, usize>::builder()
        .max_nodes(8)
        .max_output_bytes(64)
        .build();
    let mut encoder = JcsEncoder::with_limits(limits, 16 * 1024);
    let error = encoder.write_incremental(&mut RejectingWriter, &value).unwrap_err();
    assert_eq!(error.kind(), JcsEncodeErrorKind::Write);
    assert_eq!(encoder.session().value_budget().used_nodes(), Some(0));
    assert_eq!(encoder.session().output_budget().unwrap().used(), 0);
}

#[test]
fn test_jcs_numeric_finite_binary64_controls_are_unchanged() {
    if !supports_arbitrary_precision() {
        assert!(serde_json::from_str::<Value>("1e400").is_err());
        return;
    }
    for (input, expected) in [
        ("-0", "0"),
        ("1.5", "1.5"),
        ("9007199254740993", "9007199254740992"),
        ("5e-324", "5e-324"),
        ("1e-400", "0"),
    ] {
        let value: Value = serde_json::from_str(input).unwrap();
        assert_eq!(
            JcsEncoder::unlimited().to_vec(&value).unwrap(),
            expected.as_bytes(),
            "{input}"
        );
    }
}

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
    assert!(
        JcsDecoder::unlimited()
            .decode_str(r#"{"a":{"$serde_json::private::Number":"2"},"s":"ordinary"}"#)
            .unwrap()
            .is_object()
    );
    assert!(matches!(
        JcsDecoder::unlimited().decode_str(r#"{"a":{"x":1,"x":2}}"#),
        Err(JcsDecodeError::DuplicateKey { .. })
    ));
    let value = JcsDecoder::unlimited()
        .decode_str(r#"{"😀":1,"":2,"é":"ok"}"#)
        .unwrap();
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

/// Detects the actual serde_json Number model selected by unified dependency
/// features.
fn supports_arbitrary_precision() -> bool {
    match serde_json::from_str::<Value>("1e400") {
        Ok(value) => {
            assert_eq!(value.as_number().unwrap().to_string(), "1e+400");
            true
        }
        Err(error) => {
            assert!(error.is_syntax());
            false
        }
    }
}
