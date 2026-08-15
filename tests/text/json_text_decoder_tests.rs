// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests the stateful strict JSON text decoder public API.

use qubit_budget::json::JsonDecodeLimits;
use qubit_budget::json::JsonDecodeSession;
use qubit_json::text::JsonTextDecoder;

/// Verifies a decoder returns a typed value for one complete document.
#[test]
fn test_json_text_decoder_decodes_typed_value() {
    let mut session = JsonDecodeSession::owned(JsonDecodeLimits::empty());
    let value = JsonTextDecoder::new(&mut session)
        .decode::<bool>(b"true")
        .expect("JSON boolean should decode");

    assert!(value);
}
