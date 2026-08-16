// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests complete-document lexical scanning through validation.

use qubit_budget::json::JsonDecodeLimits;
use qubit_budget::json::JsonDecodeSession;
use qubit_budget::json::JsonResource;
use qubit_json::decode::JsonDecoder;

/// Verifies lexical scanning accepts one complete JSON document.
#[test]
fn test_lexical_scanner_admits_complete_document() {
    let mut session = JsonDecodeSession::owned(
        JsonDecodeLimits::<JsonResource, usize>::builder().build(),
    );

    JsonDecoder::new(&mut session)
        .validate(br#"{"items":[true,null]}"#)
        .expect("complete JSON document should validate");
}
