// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests byte-cursor behavior through strict decoder input handling.

use qubit_budget::json::JsonDecodeLimits;
use qubit_budget::json::JsonDecodeSession;
use qubit_json::text::JsonTextDecoder;

/// Verifies the lexical cursor skips JSON whitespace before and after a value.
#[test]
fn test_cursor_skips_json_whitespace() {
    let mut session = JsonDecodeSession::owned(JsonDecodeLimits::empty());
    let value = JsonTextDecoder::new(&mut session)
        .decode::<u8>(b" \n\t 7\r ")
        .expect("whitespace-wrapped JSON number should decode");

    assert_eq!(value, 7);
}
