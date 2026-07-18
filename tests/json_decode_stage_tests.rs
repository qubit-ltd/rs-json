// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests for the public `JsonDecodeStage` type in `json_decode_stage.rs`.

use qubit_json::JsonDecodeStage;

/// Verifies that decode stage display uses snake case tokens.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_stage_display_uses_snake_case_tokens() {
    assert_eq!(JsonDecodeStage::DecodeText.to_string(), "decode_text");
    assert_eq!(JsonDecodeStage::Normalize.to_string(), "normalize");
    assert_eq!(JsonDecodeStage::Parse.to_string(), "parse");
    assert_eq!(
        JsonDecodeStage::TopLevelCheck.to_string(),
        "top_level_check"
    );
    assert_eq!(JsonDecodeStage::Deserialize.to_string(), "deserialize");
}
