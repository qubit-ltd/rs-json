// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests nested container continuation through the public decoder.

use qubit_budget::json::JsonDecodeLimits;
use qubit_budget::json::JsonDecodeSession;
use qubit_budget::json::JsonResource;
use qubit_json::decode::JsonDecoder;

/// Verifies nested arrays resume their enclosing container frames.
#[test]
fn test_container_frames_resume_nested_arrays() {
    let mut session = JsonDecodeSession::owned(
        JsonDecodeLimits::<JsonResource, usize>::builder().build(),
    );
    let value = JsonDecoder::new(session)
        .decode_utf8::<serde_json::Value>(br#"[[1],2]"#)
        .expect("nested array JSON should decode");

    assert_eq!(value, serde_json::json!([[1], 2]));
}
