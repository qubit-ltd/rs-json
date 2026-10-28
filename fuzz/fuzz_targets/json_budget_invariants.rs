// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Fuzzes JSON budget invariants using an explicit control header.

#![no_main]

use libfuzzer_sys::fuzz_target;
use qubit_json_fuzz::json_budget_invariants::assert_json_budget_fuzz_input;

fuzz_target!(|data: &[u8]| {
    assert_json_budget_fuzz_input(data);
});
