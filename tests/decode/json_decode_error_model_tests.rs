// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Verifies the unified JSON decoding error model.

use std::str::FromStr;

use qubit_budget::json::JsonDecodeLimits;
use qubit_budget::json::JsonResource;
use qubit_json::decode::JsonDecodeErrorKind;
use qubit_json::decode::JsonDecodeStage;
use qubit_json::decode::JsonDecoder;
use qubit_json::decode::MarkdownFencePolicy;
use qubit_json::decode::NormalizingJsonDecodePolicy;
use qubit_json::decode::NormalizingJsonDecoder;

/// Creates a normalization policy that leaves the JSON text unchanged.
fn no_normalization_policy() -> NormalizingJsonDecodePolicy {
    NormalizingJsonDecodePolicy::builder()
        .trim_whitespace(false)
        .strip_utf8_bom(false)
        .markdown_fence_policy(MarkdownFencePolicy::Disabled)
        .escape_control_chars_in_strings(false)
        .build()
}

/// Verifies strict syntax failures use the unified parse diagnostics.
#[test]
fn test_json_decode_error_reports_strict_parse_failure() {
    let mut decoder = JsonDecoder::unlimited();
    let error = decoder
        .validate_str("{")
        .expect_err("an incomplete object must fail validation");

    assert_eq!(error.kind(), JsonDecodeErrorKind::InvalidJson);
    assert_eq!(error.stage(), JsonDecodeStage::Parse);
    assert_eq!(error.raw_input_bytes(), 1);
    assert!(error.syntax_error().is_some());
    assert!(error.budget_error().is_none());
}

/// Verifies normalizing failures use the same public kind and stage types.
#[test]
fn test_json_decode_error_reports_normalization_failure() {
    let mut decoder = NormalizingJsonDecoder::with_limits(
        NormalizingJsonDecodePolicy::lenient(),
        JsonDecodeLimits::<JsonResource, usize>::default(),
    );
    let error = decoder.decode_value("").expect_err("empty normalized input must fail");

    assert_eq!(error.kind(), JsonDecodeErrorKind::EmptyInput);
    assert_eq!(error.stage(), JsonDecodeStage::Normalize);
    assert_eq!(error.raw_input_bytes(), 0);
}

/// Verifies equivalent facade failures expose identical semantic kinds and
/// stages even though one facade has an explicit normalization phase.
#[test]
fn test_decoder_facades_share_error_classification() {
    let mut strict = JsonDecoder::unlimited();
    let mut normalizing = NormalizingJsonDecoder::with_limits(
        no_normalization_policy(),
        JsonDecodeLimits::<JsonResource, usize>::default(),
    );

    let strict_syntax = strict.decode_str::<serde_json::Value>("{").expect_err("syntax error");
    let normalizing_syntax = normalizing
        .decode_str::<serde_json::Value>("{")
        .expect_err("syntax error");
    assert_eq!(strict_syntax.kind(), normalizing_syntax.kind());
    assert_eq!(strict_syntax.stage(), normalizing_syntax.stage());

    let strict_target = strict.decode_str::<bool>("1").expect_err("target error");
    let normalizing_target = normalizing.decode_str::<bool>("1").expect_err("target error");
    assert_eq!(strict_target.kind(), normalizing_target.kind());
    assert_eq!(strict_target.stage(), normalizing_target.stage());

    let limits = JsonDecodeLimits::builder().max_input_bytes(1_usize).build();
    let mut strict = JsonDecoder::with_limits(limits);
    let mut normalizing = NormalizingJsonDecoder::with_limits(no_normalization_policy(), limits);
    let strict_budget = strict.decode_str::<bool>("true").expect_err("input budget");
    let normalizing_budget = normalizing.decode_str::<bool>("true").expect_err("input budget");
    assert_eq!(strict_budget.kind(), normalizing_budget.kind());
    assert_eq!(strict_budget.stage(), normalizing_budget.stage());
}

/// Verifies every public error kind and stage has a stable textual round trip.
#[test]
fn test_json_decode_error_kind_and_stage_round_trip() {
    let kinds = [
        (JsonDecodeErrorKind::Budget, "budget"),
        (JsonDecodeErrorKind::EmptyInput, "empty_input"),
        (JsonDecodeErrorKind::InvalidUtf8, "invalid_utf8"),
        (JsonDecodeErrorKind::InvalidJson, "invalid_json"),
        (JsonDecodeErrorKind::UnexpectedTopLevel, "unexpected_top_level"),
        (JsonDecodeErrorKind::Deserialize, "deserialize"),
    ];
    for (kind, text) in kinds {
        assert_eq!(kind.to_string(), text);
        assert_eq!(JsonDecodeErrorKind::from_str(text), Ok(kind));
    }
    assert!(JsonDecodeErrorKind::from_str("unknown").is_err());

    let stages = [
        (JsonDecodeStage::Input, "input"),
        (JsonDecodeStage::DecodeText, "decode_text"),
        (JsonDecodeStage::Normalize, "normalize"),
        (JsonDecodeStage::Admission, "admission"),
        (JsonDecodeStage::Parse, "parse"),
        (JsonDecodeStage::TopLevelCheck, "top_level_check"),
        (JsonDecodeStage::Deserialize, "deserialize"),
    ];
    for (stage, text) in stages {
        assert_eq!(stage.to_string(), text);
        assert_eq!(JsonDecodeStage::from_str(text), Ok(stage));
    }
    assert!(JsonDecodeStage::from_str("unknown").is_err());
}

/// Verifies representative failures cover semantic categories not exercised
/// by the syntax and normalization-specific tests above.
#[test]
fn test_json_decode_error_model_representative_matrix() {
    let mut invalid_utf8 = JsonDecoder::unlimited();
    let error = invalid_utf8.validate_utf8(&[0xff]).expect_err("invalid UTF-8");
    assert_eq!(
        (error.kind(), error.stage()),
        (JsonDecodeErrorKind::InvalidUtf8, JsonDecodeStage::DecodeText),
    );
    assert_eq!(error.utf8_valid_up_to(), Some(0));

    let mut decoder = NormalizingJsonDecoder::with_limits(
        NormalizingJsonDecodePolicy::lenient(),
        JsonDecodeLimits::<JsonResource, usize>::default(),
    );
    let document = decoder.prepare_str("null").expect("prepare scalar");
    let error = decoder
        .decode_precharged_object_document::<serde_json::Value>(&document)
        .expect_err("object contract");
    assert_eq!(
        (error.kind(), error.stage()),
        (JsonDecodeErrorKind::UnexpectedTopLevel, JsonDecodeStage::TopLevelCheck,),
    );

    let error = decoder
        .decode_precharged_document::<bool>(&document)
        .expect_err("target mismatch");
    assert_eq!(
        (error.kind(), error.stage()),
        (JsonDecodeErrorKind::Deserialize, JsonDecodeStage::Deserialize,),
    );
}
