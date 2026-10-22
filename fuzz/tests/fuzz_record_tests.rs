// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Regression checks for the typed decoder fuzz payload.

use serde_json::Value;
use serde_json::from_str;
use serde_json::json;

#[test]
fn test_fuzz_record_decodes_representative_object_fields() {
    let record: Value = from_str(r#"{"id":7,"label":"representative","enabled":true,"tags":["fuzz","record"]}"#)
        .expect("the corpus object must decode as a JSON object");

    assert_eq!(record["id"], 7);
    assert_eq!(record["label"], "representative");
    assert_eq!(record["enabled"], true);
    assert_eq!(record["tags"], json!(["fuzz", "record"]));
}
