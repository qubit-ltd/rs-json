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
use qubit_budget::json::JsonDecodeLimits;
use qubit_json::decode::DiagnosticPolicy;
use qubit_json::decode::JsonDecodeError;
use qubit_json::decode::JsonDecodeErrorKind;
use qubit_json::decode::JsonDecodeStage;
use qubit_json::decode::JsonDecoder;
use qubit_json::decode::MarkdownFenceClosing;
use qubit_json::decode::MarkdownFencePolicy;
use qubit_json::decode::NormalizingJsonDecodePolicy;
use qubit_json::decode::NormalizingJsonDecoder;
use qubit_json_fuzz::json_number_contract::numbers_fit_contract;

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
    assert_eq!(error.diagnostic_policy(), DiagnosticPolicy::Redacted);
    if error.kind() == JsonDecodeErrorKind::Budget {
        assert!(std::error::Error::source(error).is_some());
    } else {
        assert!(std::error::Error::source(error).is_none());
    }
    let valid_stage = match error.kind() {
        JsonDecodeErrorKind::Budget => matches!(
            error.stage(),
            JsonDecodeStage::Input | JsonDecodeStage::Normalize | JsonDecodeStage::Admission
        ),
        JsonDecodeErrorKind::EmptyInput => error.stage() == JsonDecodeStage::Normalize,
        JsonDecodeErrorKind::InvalidUtf8 => error.stage() == JsonDecodeStage::DecodeText,
        JsonDecodeErrorKind::InvalidJson => error.stage() == JsonDecodeStage::Parse,
        JsonDecodeErrorKind::UnexpectedTopLevel => error.stage() == JsonDecodeStage::TopLevelCheck,
        JsonDecodeErrorKind::Deserialize => error.stage() == JsonDecodeStage::Deserialize,
    };
    assert!(valid_stage, "error kind and stage are inconsistent: {error:?}");
}

/// Creates a normalization policy that leaves input text unchanged.
fn no_normalization_policy() -> NormalizingJsonDecodePolicy {
    NormalizingJsonDecodePolicy::builder()
        .trim_whitespace(false)
        .strip_utf8_bom(false)
        .markdown_fence_policy(MarkdownFencePolicy::Disabled)
        .escape_control_chars_in_strings(false)
        .build()
}

fuzz_target!(|data: &[u8]| {
    if !is_fuzz_input_within_limit(data) {
        return;
    }

    let mut default_decoder =
        NormalizingJsonDecoder::with_limits(NormalizingJsonDecodePolicy::default(), qubit_budget::json::JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::default());
    match default_decoder.decode_utf8::<serde_json::Value>(data) {
        Ok(value) => {
            let encoded = serde_json::to_vec(&value).expect("serde_json::Value must serialize");
            let _: serde_json::Value =
                serde_json::from_slice(&encoded).expect("successful decoder output must be strict JSON");
        }
        Err(error) => assert_error_invariants(&error, data.len()),
    }
    if !data.is_empty() {
        let mut bounded = NormalizingJsonDecoder::with_limits(
            NormalizingJsonDecodePolicy::builder().build(),
            JsonDecodeLimits::builder().max_input_bytes(data.len() - 1).build(),
        );
        let error = bounded
            .decode_utf8::<serde_json::Value>(data)
            .expect_err("an input above its raw byte limit must fail");
        assert_eq!(error.kind(), JsonDecodeErrorKind::Budget);
        assert_eq!(error.stage(), JsonDecodeStage::Input);
        assert_error_invariants(&error, data.len());
    }

    let strict_result = JsonDecoder::unlimited().decode_utf8::<serde_json::Value>(data);
    let serde_result = serde_json::from_slice::<serde_json::Value>(data);
    match (strict_result, serde_result) {
        (Ok(actual), Ok(expected)) => assert_eq!(actual, expected),
        (Err(error), Err(_)) => assert_error_invariants(&error, data.len()),
        (Ok(_), Err(_)) => {
            panic!("strict decoder accepted input rejected by serde_json");
        }
        (Err(error), Ok(_)) => {
            assert_error_invariants(&error, data.len());
            assert!(
                !numbers_fit_contract(data),
                "strict decoding must accept serde_json input whose numbers fit the public contract",
            );
        }
    }

    let Ok(input) = std::str::from_utf8(data) else {
        return;
    };

    let decoder_configurations = [
        (NormalizingJsonDecodePolicy::default(), qubit_budget::json::JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::default()),
        (no_normalization_policy(), qubit_budget::json::JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::default()),
        (
            NormalizingJsonDecodePolicy::builder()
                .markdown_fence_policy(MarkdownFencePolicy::Any {
                    closing: MarkdownFenceClosing::Optional,
                })
                .build(),
            qubit_budget::json::JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::default(),
        ),
        (
            NormalizingJsonDecodePolicy::builder()
                .markdown_fence_policy(MarkdownFencePolicy::JsonOnly {
                    closing: MarkdownFenceClosing::Required,
                })
                .build(),
            qubit_budget::json::JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::default(),
        ),
        (
            NormalizingJsonDecodePolicy::default(),
            JsonDecodeLimits::builder()
                .max_normalized_input_bytes(input.len())
                .build(),
        ),
    ];

    for (policy, limits) in decoder_configurations {
        let mut decoder = NormalizingJsonDecoder::with_limits(policy, limits);
        if let Err(error) = decoder.decode_str::<FuzzRecord>(input) {
            assert_error_invariants(&error, input.len());
        }
        if let Err(error) = decoder.decode_object_str::<FuzzRecord>(input) {
            assert_error_invariants(&error, input.len());
        }
        if let Err(error) = decoder.decode_array_str::<FuzzRecord>(input) {
            assert_error_invariants(&error, input.len());
        }
        if let Err(error) = decoder.decode_value(input) {
            assert_error_invariants(&error, input.len());
        }
    }

    if let Ok(value) = default_decoder.decode_object_str::<serde_json::Value>(input) {
        let decoded = default_decoder
            .decode_value(input)
            .expect("typed object decoding must agree with dynamic decoding");
        assert_eq!(value, decoded);
    }
    if let Ok(values) = default_decoder.decode_array_str::<serde_json::Value>(input) {
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
