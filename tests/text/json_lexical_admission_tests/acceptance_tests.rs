// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests lexical JSON admission of complete values.

use qubit_budget::json::JsonDecodeLimits;
use qubit_budget::json::JsonDecodeSession;
use qubit_json::text::JsonTextDecoder;

/// Verifies lexical admission accepts one complete JSON value.
#[test]
fn test_json_lexical_preflight_accepts_complete_value() {
    let mut session = JsonDecodeSession::owned(JsonDecodeLimits::empty());
    let value = JsonTextDecoder::new(&mut session)
        .decode::<serde_json::Value>(br#"{"ok":true}"#)
        .expect("complete JSON should pass lexical admission");

    assert_eq!(value["ok"], true);
}
