// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Fuzzes mutable JSON tree restoration and visitor-control invariants.

#![no_main]

use libfuzzer_sys::fuzz_target;
use qubit_json_fuzz::json_tree_invariants::assert_json_tree_invariants;

fuzz_target!(|data: &[u8]| {
    assert_json_tree_invariants(data);
});
