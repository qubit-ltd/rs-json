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
use qubit_json_fuzz::input_limit::bounded_input;
use qubit_json_fuzz::json_decode_differential_oracle::assert_decode_contract;

fuzz_target!(|data: &[u8]| {
    let Some(input) = bounded_input(data) else {
        return;
    };

    assert_decode_contract(input);
});
