// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests shared JSON encoding context accounting.

use qubit_budget::json::JsonEncodeLimits;
use qubit_budget::json::JsonEncodeSession;

use crate::text::json_encode_test_support::encode;

/// Verifies string payloads are charged by the shared encoding context.
#[test]
fn test_json_encode_context_charges_string_payload() {
    let limits = JsonEncodeLimits::empty().with_max_string_bytes(0);
    let mut session = JsonEncodeSession::owned(limits);

    assert!(encode(&"value", &mut session).is_err());
}
