// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Regression checks for the typed decoder fuzz payload.

#[path = "../fuzz_targets/internal/fuzz_record.rs"]
mod fuzz_record;

use fuzz_record::FuzzRecord;

#[test]
fn test_fuzz_record_decodes_representative_object_fields() {
    let record: FuzzRecord = serde_json::from_str(
        r#"{"id":7,"label":"representative","enabled":true,"tags":["fuzz","record"]}"#,
    )
    .expect("the corpus object must construct the typed record");

    assert_eq!(record.id, 7);
    assert_eq!(record.label, "representative");
    assert!(record.enabled);
    assert_eq!(record.tags, ["fuzz", "record"]);
}
