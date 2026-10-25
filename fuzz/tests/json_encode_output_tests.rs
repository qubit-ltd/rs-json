// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Regressions for the shared JSON output writer.

use std::io::Write;

use qubit_json_fuzz::json_encode_output::FuzzOutput;
use qubit_json_fuzz::json_encode_output::OutputMode;

#[test]
fn test_output_fixture_retries_interrupted_short_writes() {
    let mut output = FuzzOutput::new(OutputMode::InterruptOnce, usize::MAX, 2);
    output
        .write_all(b"record")
        .expect("interrupted short writer must complete");
    assert_eq!(output.bytes(), b"record");
}

#[test]
fn test_output_fixture_fails_after_exact_prefix() {
    let mut output = FuzzOutput::new(OutputMode::BrokenPipe, 3, 2);
    let error = output
        .write_all(b"record")
        .expect_err("writer must fail after its configured prefix");
    assert_eq!(error.kind(), std::io::ErrorKind::BrokenPipe);
    assert_eq!(output.bytes(), b"rec");
}
