/*******************************************************************************
 *
 *    Copyright (c) 2026.
 *    Haixing Hu, Qubit Co. Ltd.
 *
 *    All rights reserved.
 *
 ******************************************************************************/
//! Tests for the public `JsonDecodeStage` type in `json_decode_stage.rs`.
//!
//! Author: Haixing Hu

use qubit_json::JsonDecodeStage;

#[test]
fn test_decode_stage_display_uses_snake_case_tokens() {
    assert_eq!(JsonDecodeStage::Normalize.to_string(), "normalize");
    assert_eq!(JsonDecodeStage::Parse.to_string(), "parse");
    assert_eq!(
        JsonDecodeStage::TopLevelCheck.to_string(),
        "top_level_check"
    );
    assert_eq!(JsonDecodeStage::Deserialize.to_string(), "deserialize");
}
