// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
// =============================================================================
//! Tests the JSON decode differential-fuzz oracle.

use qubit_budget::json::JsonDecodeLimits;
use qubit_budget::json::JsonDecodeSession;
use qubit_json::decode::JsonDecodeErrorKind;
use qubit_json::decode::JsonDecoder;
use qubit_json_fuzz::json_decode_differential_oracle::assert_decode_contract;

/// Builds one valid JSON array nested to `depth` container levels.
fn nested_arrays(depth: usize) -> Vec<u8> {
    let mut input = Vec::with_capacity(depth.saturating_mul(2).saturating_add(1));
    input.extend(std::iter::repeat_n(b'[', depth));
    input.push(b'0');
    input.extend(std::iter::repeat_n(b']', depth));
    input
}

/// Verifies the oracle compares the reference parser on documents that are
/// safely inside its supported nesting domain.
#[test]
fn test_decode_contract_accepts_shallow_reference_documents() {
    assert_decode_contract(br#"{"items":[1,true,"text"]}"#);
}

/// Verifies a lexically admitted document may fail `Value` materialization at
/// Serde's recursion boundary without violating the differential contract.
#[test]
fn test_decode_contract_allows_deserialize_failure_beyond_reference_recursion_limit() {
    let input = nested_arrays(128);
    let validation_session =
        JsonDecodeSession::from_limits(JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::builder().build());
    JsonDecoder::new(validation_session)
        .validate_utf8(&input)
        .expect("deep JSON must remain lexically admissible");

    let decode_session =
        JsonDecodeSession::from_limits(JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::builder().build());
    let error = JsonDecoder::new(decode_session)
        .decode_utf8::<serde_json::Value>(&input)
        .expect_err("serde_json Value materialization must reach its recursion boundary");
    assert_eq!(error.kind(), JsonDecodeErrorKind::Deserialize);

    assert_decode_contract(&input);
}
