// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use qubit_json::decode::JcsDecodeError;
use qubit_json::decode::JcsDecoder;
use qubit_json::decode::JsonDecoder;
use qubit_json::encode::JcsEncoder;

#[test]
fn test_jcs_decoder_accepts_binary64_integer_text() {
    let input = "100000000000000000000";
    assert!(JsonDecoder::unlimited().decode_str::<serde_json::Value>(input).is_err());
    let value = JcsDecoder::unlimited().decode_str(input).unwrap();
    assert!(value.as_number().unwrap().is_f64());
    assert_eq!(JcsEncoder::unlimited().to_vec(&value).unwrap(), input.as_bytes());
}

#[test]
fn test_jcs_decoder_rejects_decoded_duplicate_key_without_leaking_key() {
    let error = JcsDecoder::unlimited()
        .decode_str(r#"{"secret":1,"\u0073ecret":2}"#)
        .unwrap_err();
    assert!(matches!(error, JcsDecodeError::DuplicateKey { .. }));
    assert!(!format!("{error:?}").contains("secret"));
    assert!(!error.to_string().contains("secret"));
    assert!(std::error::Error::source(&error).is_none());
}

#[test]
fn test_jcs_decoder_rounds_all_number_forms_to_binary64() {
    for input in ["9007199254740993", "9007199254740993.0", "9007199254740993e0"] {
        let value = JcsDecoder::unlimited().decode_str(input).unwrap();
        assert_eq!(value.as_f64().unwrap().to_bits(), 0x4340_0000_0000_0000);
    }
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
    let mut input = std::io::Cursor::new(b"100000000000000000000".to_vec());
    let value = JcsDecoder::unlimited().decode_input(&mut input).unwrap();
    assert_eq!(value.as_f64().unwrap(), 1e20);
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
    let mut decoder = JcsDecoder::unlimited().with_diagnostic_policy(qubit_json::decode::DiagnosticPolicy::Detailed);
    let error = decoder.decode_str(r#"{"secret":1,"secret":2}"#).unwrap_err();
    assert!(matches!(error, JcsDecodeError::DuplicateKey { .. }));
    assert!(!format!("{error:?}").contains("secret"));
    assert!(!error.to_string().contains("secret"));
    assert!(std::error::Error::source(&error).is_none());
}

#[test]
fn test_jcs_decoder_reports_strict_syntax_reasons() {
    use qubit_json::decode::JsonSyntaxErrorReason;

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
        assert_eq!(error.kind(), qubit_json::decode::JsonDecodeErrorKind::InvalidJson);
        assert_eq!(error.syntax_error().unwrap().reason(), expected_reason);
    }
}

#[test]
fn test_jcs_decoder_accepts_json_whitespace_and_reports_binary64_range_errors() {
    use qubit_json::decode::JsonSyntaxErrorReason;

    assert_eq!(
        JcsDecoder::unlimited().decode_str(" \n true\t ").unwrap(),
        serde_json::Value::Bool(true),
    );

    let error = JcsDecoder::unlimited().decode_str("1e400").unwrap_err();
    let JcsDecodeError::Decode(error) = error else {
        panic!("expected a structured range error")
    };
    assert_eq!(error.kind(), qubit_json::decode::JsonDecodeErrorKind::InvalidJson);
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
    assert_eq!(decoder.decode_str("true").unwrap(), serde_json::Value::Bool(true));

    let mut detailed = JcsDecoder::unlimited().with_diagnostic_policy(qubit_json::decode::DiagnosticPolicy::Detailed);
    let error = detailed.decode_str("{").unwrap_err();
    let JcsDecodeError::Decode(inner) = error else {
        panic!("expected invalid JSON decode error")
    };
    assert_eq!(inner.kind(), qubit_json::decode::JsonDecodeErrorKind::InvalidJson);
    assert!(std::error::Error::source(&inner).is_some());

    let redacted = JcsDecoder::unlimited().decode_str("{").unwrap_err();
    let JcsDecodeError::Decode(inner) = redacted else {
        panic!("expected invalid JSON decode error")
    };
    assert!(std::error::Error::source(&inner).is_none());
}
