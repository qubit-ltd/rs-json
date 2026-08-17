// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests incremental output writing through the public encoder.

use qubit_budget::json::JsonEncodeLimits;
use qubit_budget::json::JsonEncodeSession;
use qubit_budget::json::JsonResource;
use qubit_json::encode::JsonEncoder;

/// Verifies the incremental writer forwards accepted JSON bytes.
#[test]
fn test_json_output_writer_writes_incremental_document() {
    let mut session = JsonEncodeSession::owned(
        JsonEncodeLimits::<JsonResource, usize>::builder().build(),
    );
    let mut output = Vec::new();

    JsonEncoder::new(session)
        .write_incremental(&mut output, &true)
        .expect("incremental writer should accept a boolean");

    assert_eq!(output, b"true");
}
