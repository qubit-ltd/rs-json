// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests serde_json private struct shape handling through public encoding.

use qubit_budget::json::JsonEncodeLimits;
use qubit_budget::json::JsonEncodeSession;
use qubit_budget::json::JsonResource;
use qubit_json::text::JsonTextEncoder;
use serde_json::value::RawValue;

/// Verifies raw JSON values preserve their represented JSON shape.
#[test]
fn test_private_struct_kind_recognizes_raw_value() {
    let raw = RawValue::from_string(String::from("{\"ok\":true}"))
        .expect("raw JSON should parse");
    let mut session = JsonEncodeSession::owned(
        JsonEncodeLimits::<JsonResource, usize>::builder().build(),
    );
    let output = JsonTextEncoder::new(&mut session)
        .to_vec(&raw)
        .expect("raw JSON value should encode");

    assert_eq!(output, br#"{"ok":true}"#);
}
