// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use std::error::Error;
use std::io::Cursor;
use std::io::Error as IoError;
use std::io::ErrorKind;
use std::io::Result as IoResult;

use qubit_budget::json::JsonDecodeLimits;
use qubit_budget::json::JsonResource;
use qubit_budget::json::JsonValueLimits;
use qubit_io::Input;
use qubit_json::decode::DiagnosticPolicy;
use qubit_json::decode::JcsDecodeError;
use qubit_json::decode::JcsDecoder;
use qubit_json::decode::JsonDecodeErrorKind;
use qubit_json::decode::JsonDecodeStage;
use qubit_json::decode::JsonDecoder;
use qubit_json::decode::JsonSyntaxErrorReason;
use qubit_json::encode::JcsEncoder;
use qubit_json::value::SafeJsonValue;
use serde_json::Value;

struct ScriptedJcsInput {
    bytes: &'static [u8],
    position: usize,
    interrupt_once: bool,
    fail_at: Option<usize>,
    reads: usize,
}

impl ScriptedJcsInput {
    fn new(bytes: &'static [u8]) -> Self {
        Self {
            bytes,
            position: 0,
            interrupt_once: false,
            fail_at: None,
            reads: 0,
        }
    }
}

impl Input for ScriptedJcsInput {
    type Item = u8;

    unsafe fn read_unchecked(&mut self, output: &mut [u8], index: usize, count: usize) -> IoResult<usize> {
        self.reads += 1;
        if self.interrupt_once {
            self.interrupt_once = false;
            return Err(IoError::from(ErrorKind::Interrupted));
        }
        if self.fail_at == Some(self.position) {
            return Err(IoError::other("scripted terminal read failure"));
        }
        if self.position == self.bytes.len() {
            return Ok(0);
        }
        let read_limit = self.fail_at.unwrap_or(self.bytes.len());
        let read_len = count.min(read_limit - self.position);
        output[index..index + read_len].copy_from_slice(&self.bytes[self.position..self.position + read_len]);
        self.position += read_len;
        Ok(read_len)
    }
}

#[test]
fn test_jcs_decoder_accepts_binary64_integer_text() {
    let input = "100000000000000000000";
    assert!(JsonDecoder::unlimited().decode_str::<Value>(input).is_err());
    let value = JcsDecoder::unlimited().decode_str(input).unwrap();
    assert!(value.as_number().unwrap().is_f64());
    assert_eq!(JcsEncoder::unlimited().to_vec(&value).unwrap(), input.as_bytes());
}

#[test]
fn test_jcs_decoder_unlimited_does_not_measure_unconfigured_value_resources() {
    let mut decoder = JcsDecoder::unlimited();
    assert_eq!(decoder.decode_str(r#"{"value":[1,true]}"#).unwrap()["value"][1], true);
    assert_eq!(decoder.session().value_budget().used_nodes(), None);
}

#[test]
fn test_jcs_decoder_rejects_decoded_duplicate_key_without_leaking_key() {
    let error = JcsDecoder::unlimited()
        .decode_str(r#"{"secret":1,"\u0073ecret":2}"#)
        .unwrap_err();
    assert!(matches!(error, JcsDecodeError::DuplicateKey { .. }));
    assert!(!format!("{error:?}").contains("secret"));
    assert!(!error.to_string().contains("secret"));
    assert!(Error::source(&error).is_none());
}

#[test]
fn test_jcs_decoder_rounds_all_number_forms_to_binary64() {
    for input in ["9007199254740993", "9007199254740993.0", "9007199254740993e0"] {
        let value = JcsDecoder::unlimited().decode_str(input).unwrap();
        assert_eq!(value.as_f64().unwrap().to_bits(), 0x4340_0000_0000_0000);
    }
}

#[test]
fn test_jcs_decoder_preserves_number_and_real_marker_object() {
    let mut decoder = JcsDecoder::unlimited();
    for token in ["1.5", "9007199254740993.0", "9007199254740993e0"] {
        let value = decoder.decode_str(token).unwrap();
        let number = value.as_f64().expect("JSON number must remain a number");
        let expected: f64 = token.parse().unwrap();
        assert_eq!(number.to_bits(), expected.to_bits());
    }
    let object = decoder.decode_str(r#"{"$serde_json::private::Number":"1.5"}"#).unwrap();
    assert!(object.is_object());
    assert_eq!(object["$serde_json::private::Number"].as_str(), Some("1.5"));
}

#[test]
fn test_jcs_decoder_accepts_deep_values_with_shared_builder() {
    let nested = |depth: usize| format!("{}0{}", "[".repeat(depth), "]".repeat(depth));
    for depth in [128, 129] {
        let value = JcsDecoder::unlimited().decode_str(&nested(depth)).unwrap();
        let protected = SafeJsonValue::new(value);
        assert!(protected.as_value().is_array());
    }
}

#[test]
fn test_jcs_decoder_legacy_depth_variant_remains_displayable() {
    let error: JcsDecodeError = JcsDecodeError::MaterializationDepthLimit {
        limit: 127,
        line: 1,
        column: 128,
    };
    assert_eq!(
        error.to_string(),
        "JCS value construction depth exceeds 127 at line 1, column 128"
    );
    assert!(Error::source(&error).is_none());
}

#[test]
fn test_jcs_decoder_entry_points_share_binary64_value_semantics() {
    let by_str = JcsDecoder::unlimited().decode_str("1.5").unwrap();
    let by_utf8 = JcsDecoder::unlimited().decode_utf8(b"1.5").unwrap();
    let mut input = Cursor::new(b"1.5".to_vec());
    let by_input = JcsDecoder::unlimited().decode_input(&mut input).unwrap();
    assert_eq!(by_str.as_f64().unwrap().to_bits(), by_utf8.as_f64().unwrap().to_bits());
    assert_eq!(by_str.as_f64().unwrap().to_bits(), by_input.as_f64().unwrap().to_bits());
}

#[test]
fn test_jcs_decoder_input_materialization_failure_keeps_input_and_allows_reuse() {
    let limits = JsonDecodeLimits::<JsonResource, usize>::builder()
        .max_input_bytes(64)
        .max_nodes(32)
        .build();
    let mut decoder = JcsDecoder::with_limits(limits);
    let duplicate = r#"{"x":1,"x":2}"#;
    let mut failed_input = Cursor::new(duplicate.as_bytes().to_vec());
    assert!(decoder.decode_input(&mut failed_input).is_err());
    assert_eq!(decoder.session().value_budget().used_nodes(), Some(0));
    assert_eq!(decoder.session().input_budget().unwrap().used(), duplicate.len());

    let mut next_input = Cursor::new(b"true".to_vec());
    assert_eq!(decoder.decode_input(&mut next_input).unwrap(), Value::Bool(true));
    assert_eq!(decoder.session().value_budget().used_nodes(), Some(1));
    assert_eq!(decoder.session().input_budget().unwrap().used(), duplicate.len() + 4);
}

#[test]
fn test_jcs_decoder_input_retries_interrupted_read_and_commits_once() {
    let limits = JsonDecodeLimits::<JsonResource, usize>::builder()
        .max_input_bytes(16)
        .max_nodes(8)
        .build();
    let mut decoder = JcsDecoder::with_limits(limits);
    let mut input = ScriptedJcsInput::new(b"true");
    input.interrupt_once = true;

    assert_eq!(decoder.decode_input(&mut input).unwrap(), Value::Bool(true));
    assert_eq!(input.reads, 3);
    assert_eq!(input.position, 4);
    assert_eq!(decoder.session().input_budget().unwrap().used(), 4);
    assert_eq!(decoder.session().value_budget().used_nodes(), Some(1));
}

#[test]
fn test_jcs_decoder_input_read_failure_keeps_input_and_allows_reuse() {
    let limits = JsonDecodeLimits::<JsonResource, usize>::builder()
        .max_input_bytes(16)
        .max_nodes(8)
        .build();
    let mut decoder = JcsDecoder::with_limits(limits);
    let mut failed_input = ScriptedJcsInput::new(b"true");
    failed_input.fail_at = Some(4);
    let error = decoder.decode_input(&mut failed_input).unwrap_err();
    let JcsDecodeError::Decode(error) = error else {
        panic!("terminal input failure must be a decode error")
    };
    assert_eq!(error.kind(), JsonDecodeErrorKind::Read);
    assert_eq!(error.stage(), JsonDecodeStage::Input);
    assert_eq!(error.raw_input_bytes(), 4);
    assert_eq!(error.read_error_kind(), Some(ErrorKind::Other));
    assert_eq!(failed_input.reads, 2);
    assert_eq!(failed_input.position, 4);
    assert_eq!(decoder.session().input_budget().unwrap().used(), 4);
    assert_eq!(decoder.session().value_budget().used_nodes(), Some(0));

    let mut next_input = ScriptedJcsInput::new(b"true");
    assert_eq!(decoder.decode_input(&mut next_input).unwrap(), Value::Bool(true));
    assert_eq!(next_input.reads, 2);
    assert_eq!(next_input.position, 4);
    assert_eq!(decoder.session().input_budget().unwrap().used(), 8);
    assert_eq!(decoder.session().value_budget().used_nodes(), Some(1));
}

#[test]
fn test_jcs_decoder_reports_second_key_token_coordinates() {
    let input = "{\r\n  \"é\":1,\r\n  \"\\u00e9\":2}";
    let error = JcsDecoder::unlimited().decode_str(input).unwrap_err();
    assert!(matches!(error, JcsDecodeError::DuplicateKey { line: 3, column: 3 }));
}

#[test]
fn test_jcs_decoder_complete_admission_precedes_materialization_errors() {
    let limits = JsonDecodeLimits::<JsonResource, usize>::builder()
        .value_limits(JsonValueLimits::builder().max_depth(2).build())
        .build();
    let error = JcsDecoder::with_limits(limits).decode_str("[[0]]").unwrap_err();
    let JcsDecodeError::Decode(error) = error else {
        panic!("lexical depth budget must precede construction depth")
    };
    assert_eq!(error.kind(), JsonDecodeErrorKind::Budget);

    let mut decoder = JcsDecoder::unlimited();
    let error = decoder.decode_str(r#"{"x":1,"x":2,]"#).unwrap_err();
    let JcsDecodeError::Decode(error) = error else {
        panic!("whole-document syntax admission must precede duplicate-key materialization")
    };
    assert_eq!(error.kind(), JsonDecodeErrorKind::InvalidJson);

    assert!(matches!(
        decoder.decode_str(r#"{"x":1,"x":2,"tail":"valid"}"#),
        Err(JcsDecodeError::DuplicateKey { .. })
    ));
    assert_eq!(decoder.decode_str("true").unwrap(), Value::Bool(true));

    let limits = JsonDecodeLimits::<JsonResource, usize>::builder()
        .value_limits(JsonValueLimits::builder().max_number_bytes(2).build())
        .build();
    let error = JcsDecoder::with_limits(limits)
        .decode_str(r#"{"x":1,"x":2,"later":123456789}"#)
        .unwrap_err();
    let JcsDecodeError::Decode(error) = error else {
        panic!("a later lexical budget failure must precede the earlier duplicate key")
    };
    assert_eq!(error.kind(), JsonDecodeErrorKind::Budget);
}

#[test]
fn test_jcs_decoder_preserves_negative_zero_and_underflow() {
    for input in ["-0", "-0.0", "-0e0"] {
        assert_eq!(
            JcsDecoder::unlimited()
                .decode_str(input)
                .unwrap()
                .as_f64()
                .unwrap()
                .to_bits(),
            0x8000_0000_0000_0000
        );
    }
    assert_eq!(
        JcsDecoder::unlimited()
            .decode_str("5e-324")
            .unwrap()
            .as_f64()
            .unwrap()
            .to_bits(),
        1
    );
    assert_eq!(
        JcsDecoder::unlimited()
            .decode_str("1e-400")
            .unwrap()
            .as_f64()
            .unwrap()
            .to_bits(),
        0
    );
}

#[test]
fn test_jcs_decoder_rejects_non_finite_and_malformed_json() {
    for input in [&b"1e400"[..], b"01", b"+1", b"", b"[1,]", b"{}{}"] {
        assert!(matches!(
            JcsDecoder::unlimited().decode_utf8(input),
            Err(JcsDecodeError::Decode(_))
        ));
    }
}

#[test]
fn test_jcs_decoder_checks_duplicate_keys_per_decoded_object() {
    for input in [r#"{"outer":{"k":1,"k":2}}"#, r#"{"k":1,"nested":{"k":2}}"#] {
        let result = JcsDecoder::unlimited().decode_str(input);
        if input.contains(r#""k":1,"k":2"#) {
            assert!(matches!(result, Err(JcsDecodeError::DuplicateKey { .. })));
        } else {
            assert!(result.is_ok());
        }
    }
    assert!(JcsDecoder::unlimited().decode_str(r#"{"é":1,"e\u0301":2}"#).is_ok());
}

#[test]
fn test_jcs_decoder_accepts_owned_input_entry() {
    let mut input = Cursor::new(b"100000000000000000000".to_vec());
    let value = JcsDecoder::unlimited().decode_input(&mut input).unwrap();
    assert_eq!(value.as_f64().unwrap(), 1e20);
}

#[test]
fn test_jcs_decoder_exposes_policy_and_transfers_session() {
    let limits = JsonDecodeLimits::<JsonResource, usize>::builder()
        .max_input_bytes(4)
        .build();
    let mut decoder = JcsDecoder::with_limits(limits).with_diagnostic_policy(DiagnosticPolicy::Detailed);

    assert_eq!(decoder.diagnostic_policy(), DiagnosticPolicy::Detailed);
    assert_eq!(
        decoder.decode_str("true").expect("valid JCS input decodes"),
        Value::Bool(true),
    );
    assert_eq!(decoder.session_mut().input_budget().unwrap().used(), 4);

    let session = decoder.into_session();
    assert_eq!(session.input_budget().unwrap().used(), 4);
    assert_eq!(session.max_input_bytes(), Some(4));
}

#[test]
fn test_jcs_decoder_rejects_invalid_utf8_and_unpaired_surrogate() {
    assert!(matches!(
        JcsDecoder::unlimited().decode_utf8(&[0xff]),
        Err(JcsDecodeError::Decode(_))
    ));
    assert!(matches!(
        JcsDecoder::unlimited().decode_str(r#""\uDEAD""#),
        Err(JcsDecodeError::Decode(_))
    ));
    assert_eq!(JcsDecoder::unlimited().decode_str(r#""\uD834\uDD1E""#).unwrap(), "𝄞");
}

#[test]
fn test_jcs_decoder_duplicate_error_stays_redacted_with_detailed_policy() {
    let mut decoder = JcsDecoder::unlimited().with_diagnostic_policy(DiagnosticPolicy::Detailed);
    let error = decoder.decode_str(r#"{"secret":1,"secret":2}"#).unwrap_err();
    assert!(matches!(error, JcsDecodeError::DuplicateKey { .. }));
    assert!(!format!("{error:?}").contains("secret"));
    assert!(!error.to_string().contains("secret"));
    assert!(Error::source(&error).is_none());
}

#[test]
fn test_jcs_decoder_reports_strict_syntax_reasons() {
    let cases: [(&[u8], JsonSyntaxErrorReason); 4] = [
        (
            &[0xef, 0xbb, 0xbf, b't', b'r', b'u', b'e'],
            JsonSyntaxErrorReason::UnexpectedByte,
        ),
        (b"/* comment */ true", JsonSyntaxErrorReason::UnexpectedByte),
        (b"[1,]", JsonSyntaxErrorReason::UnexpectedByte),
        (b"true false", JsonSyntaxErrorReason::TrailingCharacters),
    ];
    for (input, expected_reason) in cases {
        let error = JcsDecoder::unlimited().decode_utf8(input).unwrap_err();
        let JcsDecodeError::Decode(error) = error else {
            panic!("expected strict syntax rejection")
        };
        assert_eq!(error.kind(), JsonDecodeErrorKind::InvalidJson);
        assert_eq!(error.syntax_error().unwrap().reason(), expected_reason);
    }
}

#[test]
fn test_jcs_decoder_accepts_json_whitespace_and_reports_binary64_range_errors() {
    assert_eq!(
        JcsDecoder::unlimited().decode_str(" \n true\t ").unwrap(),
        Value::Bool(true),
    );

    let error = JcsDecoder::unlimited().decode_str("1e400").unwrap_err();
    let JcsDecodeError::Decode(error) = error else {
        panic!("expected a structured range error")
    };
    assert_eq!(error.kind(), JsonDecodeErrorKind::InvalidJson);
    assert_eq!(
        error.syntax_error().unwrap().reason(),
        JsonSyntaxErrorReason::FloatOutOfRange
    );
}

#[test]
fn test_jcs_decoder_nested_duplicate_key_failure_recovers_and_preserves_source_policy() {
    let mut decoder = JcsDecoder::unlimited();
    let error = decoder.decode_str(r#"{"outer":{"x":1,"x":2}}"#).unwrap_err();
    assert!(matches!(error, JcsDecodeError::DuplicateKey { .. }));
    assert_eq!(decoder.decode_str("true").unwrap(), Value::Bool(true));

    let mut detailed = JcsDecoder::unlimited().with_diagnostic_policy(DiagnosticPolicy::Detailed);
    let error = detailed.decode_str("{").unwrap_err();
    let JcsDecodeError::Decode(inner) = error else {
        panic!("expected invalid JSON decode error")
    };
    assert_eq!(inner.kind(), JsonDecodeErrorKind::InvalidJson);
    assert!(Error::source(&inner).is_some());

    let redacted = JcsDecoder::unlimited().decode_str("{").unwrap_err();
    let JcsDecodeError::Decode(inner) = redacted else {
        panic!("expected invalid JSON decode error")
    };
    assert!(Error::source(&inner).is_none());
}

#[cfg(feature = "jcs")]
#[test]
fn test_jcs_decoder_keeps_binary64_and_accepts_deep_tree() {
    let text = format!("{}1.0{}", "[".repeat(129), "]".repeat(129));
    let value = qubit_json::decode::JcsDecoder::unlimited().decode_str(&text).unwrap();
    assert!(value.is_array());

    let error = qubit_json::decode::JcsDecoder::unlimited()
        .decode_str("1e400")
        .unwrap_err();
    let qubit_json::decode::JcsDecodeError::Decode(error) = error else {
        panic!("non-finite JCS number must be reported as a decode error");
    };
    assert_eq!(
        error.syntax_error().map(qubit_json::decode::JsonSyntaxError::reason),
        Some(qubit_json::decode::JsonSyntaxErrorReason::FloatOutOfRange)
    );
}

#[cfg(feature = "jcs")]
#[test]
fn test_jcs_binary64_isolated_from_ordinary_decode_model() {
    let jcs = qubit_json::decode::JcsDecoder::unlimited()
        .decode_str("9007199254740993")
        .unwrap();
    assert!(jcs.as_number().unwrap().is_f64());
    let ordinary: serde_json::Value = JsonDecoder::unlimited().decode_str("9007199254740993").unwrap();
    assert_eq!(ordinary.as_u64(), Some(9_007_199_254_740_993));
}
