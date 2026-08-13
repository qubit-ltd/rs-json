// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Differentially tests budget-aware JSON admission against serde_json.

#![no_main]

use libfuzzer_sys::fuzz_target;
use qubit_budget::json::JsonDecodeLimits;
use qubit_budget::json::JsonDecodeSession;
use qubit_json::text::decode_slice;
use serde_json::Value;

const MAX_INPUT_LEN: usize = 4 * 1024;

fuzz_target!(|data: &[u8]| {
    let input = &data[..data.len().min(MAX_INPUT_LEN)];
    let mut session = JsonDecodeSession::owned(JsonDecodeLimits::empty());
    let admitted = decode_slice::<Value, _, _>(input, &mut session).is_ok();
    let reference = serde_json::from_slice::<Value>(input).is_ok();

    assert_eq!(
        admitted, reference,
        "lexical admission differs from serde_json"
    );
});
