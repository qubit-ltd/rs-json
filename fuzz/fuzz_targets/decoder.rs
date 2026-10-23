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

#[path = "internal/fuzz_record.rs"]
mod fuzz_record;
mod internal;

use std::error::Error;
use std::str::from_utf8;

use libfuzzer_sys::fuzz_target;
use qubit_budget::json::JsonDecodeLimits;
use qubit_budget::json::JsonResource;
use qubit_json::decode::DiagnosticPolicy;
use qubit_json::decode::JsonDecodeError;
use qubit_json::decode::JsonDecodeErrorKind;
use qubit_json::decode::JsonDecodeStage;
use qubit_json::decode::JsonDecoder;
use qubit_json::decode::MarkdownFenceClosing;
use qubit_json::decode::MarkdownFencePolicy;
use qubit_json::decode::NormalizingJsonDecodePolicy;
use qubit_json::decode::NormalizingJsonDecoder;
use qubit_json_fuzz::input_limit::bounded_input;
use qubit_json_fuzz::json_number_contract::numbers_fit_contract;
use qubit_json_fuzz::json_reference::roundtrip_literal_value;
use serde_json::Value;
use serde_json::from_slice;

use crate::fuzz_record::FuzzRecord;

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
        assert!(Error::source(error).is_some());
    } else {
        assert!(Error::source(error).is_none());
    }
    if let Some(syntax) = error.syntax_error() {
        assert!(!syntax.to_string().contains("0x"));
        assert!(!format!("{syntax:?}").contains("byte"));
    }
    let valid_stage = match error.kind() {
        JsonDecodeErrorKind::Read => error.stage() == JsonDecodeStage::Input,
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
    let Some(data) = bounded_input(data) else {
        return;
    };

    let mut default_decoder = NormalizingJsonDecoder::with_limits(
        NormalizingJsonDecodePolicy::default(),
        JsonDecodeLimits::<JsonResource, usize>::default(),
    );
    match default_decoder.decode_utf8::<Value>(data) {
        Ok(value) => {
            let roundtripped =
                roundtrip_literal_value(&value).expect("successful decoder output must roundtrip as strict JSON");
            assert_eq!(roundtripped, value, "decoder output must roundtrip without data loss");
        }
        Err(error) => assert_error_invariants(&error, data.len()),
    }
    if !data.is_empty() {
        let mut bounded = NormalizingJsonDecoder::with_limits(
            NormalizingJsonDecodePolicy::builder().build(),
            JsonDecodeLimits::builder().max_input_bytes(data.len() - 1).build(),
        );
        let error = bounded
            .decode_utf8::<Value>(data)
            .expect_err("an input above its raw byte limit must fail");
        assert_eq!(error.kind(), JsonDecodeErrorKind::Budget);
        assert_eq!(error.stage(), JsonDecodeStage::Input);
        assert_error_invariants(&error, data.len());
    }

    let strict_result = JsonDecoder::unlimited().decode_utf8::<Value>(data);
    let serde_result = from_slice::<Value>(data);
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

    let Ok(input) = from_utf8(data) else {
        return;
    };

    if let Ok(record) = JsonDecoder::unlimited().decode_utf8::<FuzzRecord>(data) {
        let serde_record = from_slice::<FuzzRecord>(data).expect("successful strict decoding must match Serde");
        assert_eq!(record, serde_record);
    }

    let decoder_configurations = [
        (
            NormalizingJsonDecodePolicy::default(),
            JsonDecodeLimits::<JsonResource, usize>::default(),
        ),
        (
            no_normalization_policy(),
            JsonDecodeLimits::<JsonResource, usize>::default(),
        ),
        (
            NormalizingJsonDecodePolicy::builder()
                .markdown_fence_policy(MarkdownFencePolicy::Any {
                    closing: MarkdownFenceClosing::Optional,
                })
                .build(),
            JsonDecodeLimits::<JsonResource, usize>::default(),
        ),
        (
            NormalizingJsonDecodePolicy::builder()
                .markdown_fence_policy(MarkdownFencePolicy::JsonOnly {
                    closing: MarkdownFenceClosing::Required,
                })
                .build(),
            JsonDecodeLimits::<JsonResource, usize>::default(),
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

    if let Ok(value) = default_decoder.decode_object_str::<Value>(input) {
        let decoded = default_decoder
            .decode_value(input)
            .expect("typed object decoding must agree with dynamic decoding");
        assert_eq!(value, decoded);
    }
    if let Ok(values) = default_decoder.decode_array_str::<Value>(input) {
        let wrapped = Value::Array(values);
        let roundtripped = roundtrip_literal_value(&wrapped).expect("decoded array must remain strict JSON");
        assert_eq!(roundtripped, wrapped, "decoded array must roundtrip without data loss");
    }
    if input.contains("TOP_SECRET")
        && let Err(error) = default_decoder.decode_str::<FuzzRecord>(input)
    {
        assert!(!error.to_string().contains("TOP_SECRET"));
        assert!(!format!("{error:?}").contains("TOP_SECRET"));
    }
});
