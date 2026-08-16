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
use qubit_json::text::JsonTextEncoder;
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
    let mut session = JsonEncodeSession::owned(
        JsonEncodeLimits::empty().with_max_string_bytes(0),
    );

    assert!(
        JsonTextEncoder::new(&mut session)
            .to_vec(&DisplayValue)
            .is_err()
    );
}
