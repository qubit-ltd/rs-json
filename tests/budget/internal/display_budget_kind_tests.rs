// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Regression tests for display-based JSON budget categories.

use qubit_budget::json::JsonEncodeLimits;
use qubit_budget::json::JsonEncodeSession;
use qubit_json::text::encode_to_vec;
use serde::Serialize;
use serde::Serializer;

struct DisplayValue;

impl Serialize for DisplayValue {
    /// Emits text through Serde's display collection hook.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_str("display")
    }
}

/// Verifies display collection is rejected when its string budget is zero.
#[test]
fn test_display_budget_kind_checks_collected_string() {
    let limits = JsonEncodeLimits::empty().with_max_string_bytes(0);
    let mut session = JsonEncodeSession::owned(limits);

    assert!(encode_to_vec(&DisplayValue, &mut session).is_err());
}
