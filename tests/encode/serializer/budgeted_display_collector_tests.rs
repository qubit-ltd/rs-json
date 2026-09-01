// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests display collection against the public encoder budget contract.

use qubit_budget::json::JsonEncodeLimits;
use qubit_budget::json::JsonEncodeSession;
use qubit_budget::json::JsonResource;
use qubit_json::encode::JsonEncoder;
use serde::Serialize;
use serde::Serializer;

struct DisplayValue;

impl Serialize for DisplayValue {
    /// Collects its JSON string representation through Serde display support.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_str("display")
    }
}

/// Verifies collected display text is constrained by the string budget.
#[test]
fn test_budgeted_display_collector_rejects_excess_text() {
    let session = JsonEncodeSession::from_limits(
        JsonEncodeLimits::<JsonResource, usize>::builder()
            .max_string_bytes(0)
            .build(),
    );

    assert!(JsonEncoder::new(session).to_vec(&DisplayValue).is_err());
}
