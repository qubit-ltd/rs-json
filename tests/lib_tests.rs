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
    JsonDecodeError, JsonDecodeErrorKind, JsonDecodeOptions, JsonDecodeStage, JsonTopLevelKind,
    LenientJsonDecoder,
};

#[test]
fn test_lib_exports_public_types() {
    let decoder = LenientJsonDecoder::default();
    let options = JsonDecodeOptions::default();
    let kind = JsonTopLevelKind::Other;
    let error_kind = JsonDecodeErrorKind::EmptyInput;
    let error: JsonDecodeError = decoder
        .decode_value("")
        .expect_err("empty input should produce an exported error type");

    assert_eq!(decoder.options(), &options);
    assert_eq!(kind.to_string(), "other");
    assert_eq!(error.kind, error_kind);
    assert_eq!(error.stage, JsonDecodeStage::Normalize);
}
