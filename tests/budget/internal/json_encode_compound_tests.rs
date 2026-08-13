// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests for compound budget-aware JSON serialization.

use qubit_budget::json::JsonEncodeLimits;
use qubit_budget::json::JsonEncodeSession;
use qubit_json::text::encode_to_vec;
use serde::Serialize;

#[derive(Serialize)]
struct Pair {
    left: u8,
    right: u8,
}

/// Verifies compound serialization preserves object members.
#[test]
fn test_json_encode_compound_serializes_struct_members() {
    let mut session = JsonEncodeSession::owned(JsonEncodeLimits::empty());
    let output = encode_to_vec(&Pair { left: 1, right: 2 }, &mut session)
        .expect("compound JSON should serialize");

    assert_eq!(output, br#"{"left":1,"right":2}"#);
}
