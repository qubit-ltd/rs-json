// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use qubit_budget::json::JsonDecodeLimits;
use qubit_budget::json::JsonDecodeSession;
use qubit_json::text::decode_slice;

/// Verifies that strict text decoding uses the operation-specific error API.
#[test]
fn test_decode_slice_returns_a_typed_value() {
    let mut session = JsonDecodeSession::owned(JsonDecodeLimits::empty());
    let value: bool =
        decode_slice(b"true", &mut session).expect("valid JSON decodes");
    assert!(value);
}
