// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Shared support for the `qubit-json` fuzz targets.

pub mod fuzz_record;
pub mod input_limit;
pub mod jcs_decode_invariants;
pub mod json_budget_invariants;
pub mod json_decode_differential_oracle;
pub mod json_encode_invariants;
pub mod json_encode_output;
pub mod json_number_contract;
pub mod json_reference;
pub mod json_serde_protocol_invariants;
pub mod json_tree_invariants;
pub mod json_unique_decode_invariants;
pub mod unique_decode_reference;
