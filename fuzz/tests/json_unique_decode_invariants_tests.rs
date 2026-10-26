// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Deterministic regressions for unique-key JSON decode invariants.

use qubit_json::decode::JsonDecodeErrorKind;
use qubit_json::decode::JsonDecoder;
use qubit_json::decode::JsonMaterializationErrorReason;
use qubit_json::value::DuplicateKeyRejectingJsonValue;
use qubit_json_fuzz::json_unique_decode_invariants::assert_unique_decode_invariants;

/// Checks representative valid, invalid, duplicate, boundary, and budgeted
/// inputs.
#[test]
fn test_unique_decode_invariants_cover_fixed_contract_matrix() {
    let inputs: &[&[u8]] = &[
        br#"{"x":1,"\u0078":2}"#,
        br#"{"outer":{"x":1,"x":2}}"#,
        b"{\"unique-decode-sensitive-marker-7f3a\":1,\"unique-decode-sensitive-marker-7f3a\":2}",
        br#"{"$serde_json::private::Number":"1.5","$serde_json::private::RawValue":"{\"n\":2}"}"#,
        br#"[-9223372036854775808,18446744073709551615,-0,-0.0,1.5]"#,
        b"[9223372036854775808,-9223372036854775809,1e400]",
        b"\xff",
        b"null trailing",
        "{\"键\":1,\"键\":2}\n".as_bytes(),
        b"{\"line\":1,\r\n\"line\":2}",
    ];
    for input in inputs {
        assert_unique_decode_invariants(input, 0);
        assert_unique_decode_invariants(input, input.len());
    }
}

/// Checks the recursive materialization boundary with fixed shallow and deep
/// arrays.
#[test]
fn test_unique_decode_invariants_cover_depth_boundary() {
    let nested_arrays = |depth: usize| {
        let mut input = Vec::with_capacity(depth.saturating_mul(2).saturating_add(1));
        input.extend(std::iter::repeat_n(b'[', depth));
        input.push(b'0');
        input.extend(std::iter::repeat_n(b']', depth));
        input
    };

    for depth in [127, 128] {
        let input = nested_arrays(depth);
        assert_unique_decode_invariants(&input, 0);
        assert_unique_decode_invariants(&input, input.len());
        let result = JsonDecoder::unlimited().decode_unique_utf8::<DuplicateKeyRejectingJsonValue>(&input);
        if depth == 127 {
            result.expect("127 nested containers must materialize");
        } else {
            assert_eq!(
                result
                    .expect_err("128 nested containers must exceed the construction limit")
                    .materialization_error_reason(),
                Some(JsonMaterializationErrorReason::DepthLimit { limit: 127 }),
            );
        }
    }
}

/// Verifies duplicate, UTF-8, syntax, and numeric failures retain stable
/// classifications.
#[test]
fn test_unique_decode_invariants_classify_fixed_failures() {
    let duplicate = JsonDecoder::unlimited()
        .decode_unique_utf8::<DuplicateKeyRejectingJsonValue>(br#"{"x":1,"\u0078":2}"#)
        .expect_err("decoded duplicate keys must be rejected");
    assert_eq!(
        duplicate.materialization_error_reason(),
        Some(JsonMaterializationErrorReason::DuplicateKey)
    );

    for input in [b"\xff".as_slice()] {
        let error = JsonDecoder::unlimited()
            .decode_unique_utf8::<DuplicateKeyRejectingJsonValue>(input)
            .expect_err("invalid UTF-8 must be rejected");
        assert_eq!(error.kind(), JsonDecodeErrorKind::InvalidUtf8);
        assert_eq!(error.materialization_error_reason(), None);
    }
    for input in [
        b"null trailing".as_slice(),
        b"1e400",
        b"18446744073709551616",
        b"-9223372036854775809",
    ] {
        let error = JsonDecoder::unlimited()
            .decode_unique_utf8::<DuplicateKeyRejectingJsonValue>(input)
            .expect_err("invalid syntax or out-of-contract numbers must be rejected");
        assert_eq!(error.kind(), JsonDecodeErrorKind::InvalidJson);
        assert_eq!(error.materialization_error_reason(), None);
    }

    let oversized_integer = vec![b'1'; 62];
    assert_unique_decode_invariants(&oversized_integer, oversized_integer.len());
}
