// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests for lexical JSON admission.

use qubit_json::JsonDecodeLimits;
use qubit_json::JsonDecodeSession;
use qubit_json::decode_slice;

/// Verifies lexical admission accepts one complete JSON value.
#[test]
fn test_json_lexical_preflight_accepts_complete_value() {
    let mut session = JsonDecodeSession::owned(JsonDecodeLimits::empty());
    let value = decode_slice::<serde_json::Value, _, _>(
        br#"{"ok":true}"#,
        &mut session,
    )
    .expect("complete JSON should pass lexical admission");

    assert_eq!(value["ok"], true);
}
