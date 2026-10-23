// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Regression checks for the typed decoder fuzz payload.

use qubit_json::decode::JsonDecoder;
use qubit_json_fuzz::fuzz_record::FuzzRecord;
use serde_json::from_slice;

#[test]
fn test_fuzz_record_decodes_representative_object_fields() {
    let input = include_bytes!("../corpus/decoder/typed-fuzz-record");
    let actual: FuzzRecord = JsonDecoder::unlimited()
        .decode_utf8(input)
        .expect("the real typed corpus must reach the decoder success path");
    let expected: FuzzRecord = from_slice(input).expect("the same corpus must decode with Serde");
    assert_eq!(actual, expected);
    assert_eq!(actual.id, 7);
    assert_eq!(actual.label, "representative");
    assert!(actual.enabled);
    assert_eq!(actual.tags, vec![String::from("fuzz"), String::from("record")]);
}
