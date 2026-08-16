// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests for the public `NormalizingJsonDecodeStage` type.

use qubit_json::decode::NormalizingJsonDecodeStage;

/// Verifies that decode stage display uses snake case tokens.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_stage_display_uses_snake_case_tokens() {
    assert_eq!(
        NormalizingJsonDecodeStage::DecodeText.to_string(),
        "decode_text"
    );
    assert_eq!(
        NormalizingJsonDecodeStage::Normalize.to_string(),
        "normalize"
    );
    assert_eq!(
        NormalizingJsonDecodeStage::Admission.to_string(),
        "admission"
    );
    assert_eq!(NormalizingJsonDecodeStage::Parse.to_string(), "parse");
    assert_eq!(
        NormalizingJsonDecodeStage::TopLevelCheck.to_string(),
        "top_level_check"
    );
    assert_eq!(
        NormalizingJsonDecodeStage::Deserialize.to_string(),
        "deserialize"
    );
}
