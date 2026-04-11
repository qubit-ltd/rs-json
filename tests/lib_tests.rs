/*******************************************************************************
 *
 *    Copyright (c) 2026.
 *    Haixing Hu, Qubit Co. Ltd.
 *
 *    All rights reserved.
 *
 ******************************************************************************/
//! Smoke tests for crate-level exports in `lib.rs`.
//!
//! Author: Haixing Hu

use qubit_json::{
    JsonDecodeError, JsonDecodeErrorKind, JsonDecodeOptions, JsonTopLevelKind, LenientJsonDecoder,
    LenientJsonNormalizer,
};

#[test]
fn test_lib_exports_public_types() {
    let decoder = LenientJsonDecoder::default();
    let options = JsonDecodeOptions::default();
    let kind = JsonTopLevelKind::Other;
    let error_kind = JsonDecodeErrorKind::EmptyInput;
    let normalizer = LenientJsonNormalizer::default();
    let error = JsonDecodeError {
        kind: error_kind,
        message: "msg".to_string(),
        expected_top_level: None,
        actual_top_level: None,
        line: None,
        column: None,
    };

    assert_eq!(decoder.options(), &options);
    assert_eq!(kind.to_string(), "other");
    assert_eq!(normalizer.options(), &options);
    assert_eq!(error.kind, JsonDecodeErrorKind::EmptyInput);
}
