// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Fuzzes unique-key JSON decoding and its transaction invariants.

#![no_main]

use libfuzzer_sys::fuzz_target;
use qubit_json_fuzz::input_limit::bounded_input;
use qubit_json_fuzz::json_unique_decode_invariants::assert_unique_decode_invariants;

fuzz_target!(|data: &[u8]| {
    let Some(input) = bounded_input(data) else {
        return;
    };
    assert_unique_decode_invariants(input, data.len());
});
