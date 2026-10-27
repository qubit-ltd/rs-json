// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Fixed cross-path samples for the extended JSON fuzz invariants.

use qubit_json::decode::JsonDecodeErrorKind;
use qubit_json::decode::JsonDecoder;
use qubit_json::decode::JsonNumberPolicy;
use qubit_json::decode::JsonValidatedDecodeOptions;
use qubit_json_fuzz::json_encode_invariants::assert_json_encode_invariants;
use qubit_json_fuzz::json_unique_decode_invariants::assert_protected_decode_invariants;
use qubit_json_fuzz::json_unique_decode_invariants::assert_token_format_invariants;
use qubit_json_fuzz::json_unique_decode_invariants::assert_unique_decode_invariants;
use qubit_json_fuzz::json_unique_decode_invariants::assert_validated_limit_invariants;

#[test]
fn test_extended_invariants_cover_fixed_decode_and_token_matrix() {
    assert_validated_limit_invariants();
    let accepted = b"[9007199254740991,9007199254740992,-9007199254740991,-9007199254740992,1.00e2,0.1,-0]";
    let rejected = b"[1e999999]";
    JsonDecoder::unlimited()
        .decode_value_utf8(accepted)
        .expect("the accepted numeric matrix must decode successfully");
    let rejected_error = JsonDecoder::unlimited()
        .decode_value_utf8(rejected)
        .expect_err("the oversized exponent sample must be rejected");
    assert_eq!(rejected_error.kind(), JsonDecodeErrorKind::InvalidJson);

    let options = JsonValidatedDecodeOptions::builder().build();
    let mut tokens = Vec::new();
    JsonDecoder::unlimited()
        .decode_unique_validated_utf8::<std::convert::Infallible, _>(accepted, &options, |_, token| {
            tokens.push(token.raw().to_owned());
            Ok(JsonNumberPolicy::Preserve)
        })
        .expect("Preserve validation must accept the numeric matrix");
    assert_eq!(
        tokens,
        [
            "9007199254740991",
            "9007199254740992",
            "-9007199254740991",
            "-9007199254740992",
            "1.00e2",
            "0.1",
            "-0",
        ],
        "validated decoding must visit every original number token in order",
    );

    let samples: &[&[u8]] = &[
        br#"[1,{"x":2},3]"#,
        br#"{"a\u0062":[4,5]}"#,
        br#"{"escaped\"key":1,"nested":[{"n":2}],"tail":3}"#,
        br#"{"x":1,"\u0078":2}"#,
        br#"{"$serde_json::private::Number":"1.5"}"#,
        accepted,
        rejected,
        b"[ \t1,\r\n { \"x\" : \"a \\\" b\" } , 3 ]",
        b"[0,0,0,0,]",
        b"null trailing",
    ];
    for sample in samples {
        assert_protected_decode_invariants(sample);
        assert_token_format_invariants(sample);
        assert_unique_decode_invariants(sample, sample.len());
    }
}

#[test]
fn test_extended_invariants_cover_encode_transaction_projection_inputs() {
    for (selector, json) in [
        (0_u8, br#"[1,{"x":2},3]"#.as_slice()),
        (1, br#"{"escaped\"key":"a\nb"}"#),
        (2, b"[9007199254740991,1.00e2,-0.0]"),
        (3, br#"{"$serde_json::private::Number":"1.5"}"#),
    ] {
        let mut input = vec![0; 9];
        input[0] = selector;
        input.extend_from_slice(json);
        assert_json_encode_invariants(&input);
    }
}
