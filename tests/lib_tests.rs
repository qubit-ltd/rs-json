// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Smoke tests for crate-level exports in `lib.rs`.

use qubit_json::{
    ErrorPrivacyPolicy,
    JsonDecodeError,
    JsonDecodeErrorKind,
    JsonDecodeOptions,
    JsonDecodeStage,
    JsonTopLevelKind,
    LenientJsonDecoder,
};

/// Verifies that the crate re-exports its documented public API types.
///
/// # Panics
///
/// Panics when an exported type does not preserve its documented behavior.
#[test]
fn test_lib_exports_public_types() {
    let decoder = LenientJsonDecoder::default();
    let options = JsonDecodeOptions::default();
    let kind = JsonTopLevelKind::Other;
    let error_kind = JsonDecodeErrorKind::EmptyInput;
    let privacy_policy = ErrorPrivacyPolicy::Redacted;
    let error: JsonDecodeError = decoder
        .decode_value("")
        .expect_err("empty input should produce an exported error type");

    assert_eq!(decoder.options(), &options);
    assert_eq!(kind.to_string(), "other");
    assert_eq!(error.kind(), error_kind);
    assert_eq!(error.stage(), JsonDecodeStage::Normalize);
    assert_eq!(privacy_policy, ErrorPrivacyPolicy::default());
}

/// Verifies that pull-request CI enforces the repository's coverage thresholds.
///
/// # Panics
///
/// Panics when the workflow does not download and validate the coverage report.
#[test]
fn test_ci_workflow_enforces_coverage_thresholds() {
    let workflow = include_str!("../.github/workflows/ci.yml");

    assert!(workflow.contains("coverage-thresholds:"));
    assert!(workflow.contains("needs: rust-ci"));
    assert!(workflow.contains("actions/download-artifact@v8"));
    assert!(workflow.contains("name: coverage-reports"));
    assert!(workflow.contains("functions.percent < 100"));
    assert!(workflow.contains("lines.percent <= 95"));
    assert!(workflow.contains("regions.percent <= 95"));
}

/// Verifies that scheduled fuzzing preserves its corpus, bounds execution, and
/// retains failure artifacts.
///
/// # Panics
///
/// Panics when the workflow omits the corpus cache, execution bounds, or
/// decoder failure artifacts.
#[test]
fn test_fuzz_workflow_preserves_decoder_corpus() {
    let workflow = include_str!("../.github/workflows/fuzz.yml");

    assert!(workflow.contains("timeout-minutes: 15"));
    assert!(workflow.contains("actions/cache/restore@v4"));
    assert!(workflow.contains("actions/cache/save@v4"));
    assert!(workflow.contains("fuzz/corpus/decoder"));
    assert!(workflow.contains("github.run_id"));
    assert!(workflow.contains("if: always()"));
    assert!(workflow.contains("-max_len=4096"));
    assert!(workflow.contains("if: failure()"));
    assert!(workflow.contains("actions/upload-artifact@v7"));
    assert!(workflow.contains("fuzz/artifacts/**"));
}
