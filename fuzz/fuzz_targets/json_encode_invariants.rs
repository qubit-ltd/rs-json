// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Fuzzes JSON encoding output and accounting invariants.

#![no_main]

use libfuzzer_sys::fuzz_target;
use qubit_json_fuzz::json_encode_invariants::assert_json_encode_invariants;
use qubit_json_fuzz::json_serde_protocol_invariants::assert_json_serde_protocol_invariants;

fuzz_target!(|data: &[u8]| {
    assert_json_encode_invariants(data);
    assert_json_serde_protocol_invariants(data);
});
