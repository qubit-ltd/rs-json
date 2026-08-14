// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Smoke tests for crate-level exports in `lib.rs`.

use qubit_json::lenient::ErrorPrivacyPolicy;
use qubit_json::lenient::JsonDecodeErrorKind;
use qubit_json::lenient::JsonDecodeOptions;
use qubit_json::lenient::JsonDecodeStage;
use qubit_json::lenient::JsonTopLevelKind;
use qubit_json::lenient::LenientJsonDecodeError;
use qubit_json::lenient::LenientJsonDecoder;

use crate::fixtures::MAX_FUZZ_INPUT_BYTES;
use crate::fixtures::is_fuzz_input_within_limit;

/// Verifies that the lenient module exposes its documented public API types.
///
/// # Panics
///
/// Panics when an exported type does not preserve its documented behavior.
#[test]
fn test_lenient_exports_public_types() {
    let decoder = LenientJsonDecoder::default();
    let options = JsonDecodeOptions::default();
    let kind = JsonTopLevelKind::Other;
    let error_kind = JsonDecodeErrorKind::EmptyInput;
    let privacy_policy = ErrorPrivacyPolicy::Redacted;
    let error: LenientJsonDecodeError = decoder
        .decode_value("")
        .expect_err("empty input should produce an exported error type");

    assert_eq!(decoder.options(), &options);
    assert_eq!(kind.to_string(), "other");
    assert_eq!(error.kind(), error_kind);
    assert_eq!(error.stage(), JsonDecodeStage::Normalize);
    assert_eq!(privacy_policy, ErrorPrivacyPolicy::default());
}

/// Verifies that pull-request CI delegates coverage enforcement to the shared
/// workflow.
///
/// # Panics
///
/// Panics when the workflow does not enable shared coverage enforcement.
#[test]
fn test_ci_workflow_enforces_coverage_thresholds() {
    let workflow = include_str!("../.github/workflows/ci.yml");

    assert!(
        workflow.contains(
            "uses: qubit-ltd/rs-ci/.github/workflows/rust-ci.yml@main"
        )
    );
    assert!(workflow.contains("coverage_enforce_thresholds: \"1\""));
    assert!(!workflow.contains("coverage-thresholds:"));
}

/// Verifies that scheduled fuzzing preserves its corpus, bounds execution, and
/// retains failure artifacts.
///
/// # Panics
///
/// Panics when the workflow omits the corpus cache, execution bounds, or
/// target-specific failure artifacts.
#[test]
fn test_fuzz_workflow_preserves_each_target_corpus() {
    let workflow = include_str!("../.github/workflows/fuzz.yml");

    assert!(workflow.contains("timeout-minutes: 15"));
    assert!(workflow.contains("actions/cache/restore@v4"));
    assert!(workflow.contains("actions/cache/save@v4"));
    assert!(workflow.contains("target: [decoder, json_decode_differential, json_budget_invariants, json_encode_invariants, json_tree_invariants]"));
    assert!(workflow.contains("fuzz/corpus/${{ matrix.target }}"));
    assert!(workflow.contains("github.run_id"));
    assert!(workflow.contains("if: always()"));
    assert!(workflow.contains("-max_len=4096"));
    assert!(workflow.contains("if: failure()"));
    assert!(workflow.contains("actions/upload-artifact@v7"));
    assert!(workflow.contains("fuzz/artifacts/${{ matrix.target }}/**"));
}

/// Verifies that both README files use the pinned local fuzz toolchain.
///
/// # Panics
///
/// Panics when either README drifts from the scheduled fuzz workflow's
/// toolchain, `cargo-fuzz` version, or bounded decoder command.
#[test]
fn test_readme_fuzz_commands_match_workflow_contract() {
    let readmes = [
        include_str!("../README.md"),
        include_str!("../README.zh_CN.md"),
    ];

    for readme in readmes {
        assert!(readme.contains(
            "rustup toolchain install nightly-2026-06-05 --profile minimal",
        ));
        assert!(
            readme
                .contains("cargo install cargo-fuzz --version 0.13.2 --locked")
        );
        assert!(
            readme.contains("cargo +nightly-2026-06-05 fuzz build decoder")
        );
        assert!(readme.contains(
            "cargo +nightly-2026-06-05 fuzz run decoder -- -max_len=4096",
        ));
    }
}

/// Verifies that the decoder fuzz target accepts its exact input bound.
///
/// # Panics
///
/// Panics when the shared fuzz input boundary changes or is not inclusive.
#[test]
fn test_decoder_fuzz_input_limit_includes_exact_boundary() {
    assert_eq!(MAX_FUZZ_INPUT_BYTES, 4_096);
    assert!(is_fuzz_input_within_limit(&vec![0; 4_095]));
    assert!(is_fuzz_input_within_limit(&vec![0; 4_096]));
    assert!(!is_fuzz_input_within_limit(&vec![0; 4_097]));
}
