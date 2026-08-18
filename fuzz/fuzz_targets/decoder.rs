// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
#![no_main]
//! Exercises decoder acceptance, error-model, privacy, and shape invariants
//! over arbitrary byte input.

mod internal;

use internal::fuzz_input_limit::is_fuzz_input_within_limit;
use internal::fuzz_record::FuzzRecord;
use libfuzzer_sys::fuzz_target;
use qubit_json::decode::DiagnosticPolicy;
use qubit_json::decode::MarkdownFenceClosing;
use qubit_json::decode::MarkdownFencePolicy;
use qubit_json::decode::NormalizingJsonDecodeError as JsonDecodeError;
use qubit_json::decode::NormalizingJsonDecodeErrorKind;
use qubit_json::decode::NormalizingJsonDecodeOptions;
use qubit_json::decode::NormalizingJsonDecodeStage;
use qubit_json::decode::NormalizingJsonDecoder;

/// Verifies stable diagnostics shared by every redacted decoder configuration.
///
/// # Parameters
///
/// * `error` - Decoder error whose public invariants are checked.
/// * `raw_input_bytes` - Expected raw input length in bytes.
///
/// # Panics
///
/// Panics when the error exposes inconsistent metadata, stage mapping, privacy,
/// or source retention.
fn assert_error_invariants(error: &JsonDecodeError, raw_input_bytes: usize) {
    assert_eq!(error.raw_input_bytes(), raw_input_bytes);
    assert_eq!(error.privacy_policy(), DiagnosticPolicy::Redacted);
    assert!(std::error::Error::source(error).is_none());
    let expected_stage = match error.kind() {
        NormalizingJsonDecodeErrorKind::InputTooLarge | NormalizingJsonDecodeErrorKind::EmptyInput => {
            NormalizingJsonDecodeStage::Normalize
        }
        NormalizingJsonDecodeErrorKind::InvalidUtf8 => NormalizingJsonDecodeStage::DecodeText,
        NormalizingJsonDecodeErrorKind::InvalidJson => NormalizingJsonDecodeStage::Parse,
        NormalizingJsonDecodeErrorKind::UnexpectedTopLevel => NormalizingJsonDecodeStage::TopLevelCheck,
        NormalizingJsonDecodeErrorKind::Deserialize => NormalizingJsonDecodeStage::Deserialize,
        _ => return,
    };
    assert_eq!(error.stage(), expected_stage);
}

fuzz_target!(|data: &[u8]| {
    if !is_fuzz_input_within_limit(data) {
        return;
    }

    let mut default_decoder = NormalizingJsonDecoder::default();
    match default_decoder.decode_utf8::<serde_json::Value>(data) {
        Ok(value) => {
            let encoded = serde_json::to_vec(&value).expect("serde_json::Value must serialize");
            let _: serde_json::Value =
                serde_json::from_slice(&encoded).expect("successful decoder output must be strict JSON");
        }
        Err(error) => assert_error_invariants(&error, data.len()),
    }
    if !data.is_empty() {
        let mut bounded = NormalizingJsonDecoder::new(
            NormalizingJsonDecodeOptions::builder()
                .max_input_bytes(Some(data.len() - 1))
                .build(),
        );
        let error = bounded
            .decode_utf8::<serde_json::Value>(data)
            .expect_err("an input above its raw byte limit must fail");
        assert_eq!(error.kind(), NormalizingJsonDecodeErrorKind::InputTooLarge);
        assert_error_invariants(&error, data.len());
    }

    let strict_result =
        NormalizingJsonDecoder::new(NormalizingJsonDecodeOptions::strict()).decode_utf8::<serde_json::Value>(data);
    let serde_result = serde_json::from_slice::<serde_json::Value>(data);
    match (strict_result, serde_result) {
        (Ok(actual), Ok(expected)) => assert_eq!(actual, expected),
        (Err(error), Err(_)) => assert_error_invariants(&error, data.len()),
        (Ok(_), Err(_)) => {
            panic!("strict decoder accepted input rejected by serde_json");
        }
        (Err(_), Ok(_)) => {
            panic!("strict decoder rejected input accepted by serde_json");
        }
    }

    let Ok(input) = std::str::from_utf8(data) else {
        return;
    };

    let decoder_options = [
        NormalizingJsonDecodeOptions::default(),
        NormalizingJsonDecodeOptions::strict(),
        NormalizingJsonDecodeOptions::builder()
            .markdown_fence_policy(MarkdownFencePolicy::Any {
                closing: MarkdownFenceClosing::Optional,
            })
            .build(),
        NormalizingJsonDecodeOptions::builder()
            .markdown_fence_policy(MarkdownFencePolicy::JsonOnly {
                closing: MarkdownFenceClosing::Required,
            })
            .build(),
        NormalizingJsonDecodeOptions::builder()
            .max_normalized_bytes(Some(input.len()))
            .build(),
    ];

    for options in decoder_options {
        let mut decoder = NormalizingJsonDecoder::new(options);
        if let Err(error) = decoder.decode_str::<FuzzRecord>(input) {
            assert_error_invariants(&error, input.len());
        }
        if let Err(error) = decoder.decode_object::<FuzzRecord>(input) {
            assert_error_invariants(&error, input.len());
        }
        if let Err(error) = decoder.decode_array::<FuzzRecord>(input) {
            assert_error_invariants(&error, input.len());
        }
        if let Err(error) = decoder.decode_value(input) {
            assert_error_invariants(&error, input.len());
        }
    }

    if let Ok(value) = default_decoder.decode_object::<serde_json::Value>(input) {
        assert!(value.is_object());
    }
    if let Ok(values) = default_decoder.decode_array::<serde_json::Value>(input) {
        let encoded = serde_json::to_vec(&values).expect("decoded array elements must serialize");
        let reparsed: serde_json::Value =
            serde_json::from_slice(&encoded).expect("decoded array must remain strict JSON");
        assert!(reparsed.is_array());
    }
    if input.contains("TOP_SECRET")
        && let Err(error) = default_decoder.decode_str::<FuzzRecord>(input)
    {
        assert!(!error.to_string().contains("TOP_SECRET"));
        assert!(!format!("{error:?}").contains("TOP_SECRET"));
    }
});
