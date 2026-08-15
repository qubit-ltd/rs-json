// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests for the public `LenientJsonDecodeStage` type.

use qubit_json::lenient::LenientJsonDecodeStage;

/// Verifies that decode stage display uses snake case tokens.
///
/// # Panics
///
/// Panics when the expected behavior is not observed.
#[test]
fn test_decode_stage_display_uses_snake_case_tokens() {
    assert_eq!(
        LenientJsonDecodeStage::DecodeText.to_string(),
        "decode_text"
    );
    assert_eq!(LenientJsonDecodeStage::Normalize.to_string(), "normalize");
    assert_eq!(LenientJsonDecodeStage::Admission.to_string(), "admission");
    assert_eq!(LenientJsonDecodeStage::Parse.to_string(), "parse");
    assert_eq!(
        LenientJsonDecodeStage::TopLevelCheck.to_string(),
        "top_level_check"
    );
    assert_eq!(
        LenientJsonDecodeStage::Deserialize.to_string(),
        "deserialize"
    );
}
