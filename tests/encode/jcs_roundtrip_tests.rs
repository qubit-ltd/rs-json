// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use qubit_json::decode::JcsDecoder;
use qubit_json::encode::JcsEncoder;
use serde_json::Number;
use serde_json::Value;

#[test]
fn test_jcs_roundtrip_reads_encoder_integer_boundary_output() {
    let first = JcsEncoder::unlimited()
        .to_vec(&Value::from(u64::MAX))
        .expect("the encoder should serialize the u64 boundary fixture");
    assert_eq!(
        first, b"18446744073709552000",
        "JCS must encode the rounded binary64 value for u64::MAX"
    );
    let decoded = JcsDecoder::unlimited()
        .decode_utf8(&first)
        .expect("the encoded integer boundary must decode");
    let second = JcsEncoder::unlimited()
        .to_vec(&decoded)
        .expect("the decoded integer boundary must re-encode");
    assert_eq!(second, first, "JCS integer boundary encoding must round trip");
}

#[test]
fn test_jcs_roundtrip_fixed_rfc_binary64_vector() {
    let value = Value::Number(
        Number::from_f64(f64::from_bits(0x4430_0000_0000_0000))
            .expect("the RFC vector must be a finite binary64 value"),
    );
    let expected = b"295147905179352830000";
    let first = JcsEncoder::unlimited()
        .to_vec(&value)
        .expect("the encoder should serialize the RFC binary64 vector");
    assert_eq!(first, expected, "JCS must emit the RFC binary64 decimal form");
    let decoded = JcsDecoder::unlimited()
        .decode_utf8(expected)
        .expect("the RFC binary64 decimal form must decode");
    assert_eq!(
        decoded
            .as_f64()
            .expect("the decoded RFC vector must remain a floating-point number")
            .to_bits(),
        0x4430_0000_0000_0000,
        "decoding the RFC vector must preserve its binary64 value"
    );
    assert_eq!(
        JcsEncoder::unlimited()
            .to_vec(&decoded)
            .expect("the decoded RFC vector must re-encode"),
        expected,
        "re-encoding the RFC vector must preserve its canonical decimal form"
    );
}

#[test]
fn test_jcs_roundtrip_rfc8785_appendix_b_vectors() {
    let cases: &[(u64, &[u8])] = &[
        (0x0000_0000_0000_0000, b"0"),
        (0x8000_0000_0000_0000, b"0"),
        (0x0000_0000_0000_0001, b"5e-324"),
        (0x8000_0000_0000_0001, b"-5e-324"),
        (0x7fef_ffff_ffff_ffff, b"1.7976931348623157e+308"),
        (0xffef_ffff_ffff_ffff, b"-1.7976931348623157e+308"),
        (0x4340_0000_0000_0000, b"9007199254740992"),
        (0xc340_0000_0000_0000, b"-9007199254740992"),
        (0x4430_0000_0000_0000, b"295147905179352830000"),
        (0x44b5_2d02_c7e1_4af5, b"9.999999999999997e+22"),
        (0x44b5_2d02_c7e1_4af6, b"1e+23"),
        (0x44b5_2d02_c7e1_4af7, b"1.0000000000000001e+23"),
        (0x444b_1ae4_d6e2_ef4e, b"999999999999999700000"),
        (0x444b_1ae4_d6e2_ef4f, b"999999999999999900000"),
        (0x444b_1ae4_d6e2_ef50, b"1e+21"),
        (0x3eb0_c6f7_a0b5_ed8c, b"9.999999999999997e-7"),
        (0x3eb0_c6f7_a0b5_ed8d, b"0.000001"),
        (0x41b3_de43_5555_5553, b"333333333.3333332"),
        (0x41b3_de43_5555_5554, b"333333333.33333325"),
        (0x41b3_de43_5555_5555, b"333333333.3333333"),
        (0x41b3_de43_5555_5556, b"333333333.3333334"),
        (0x41b3_de43_5555_5557, b"333333333.33333343"),
        (0xbecb_f647_612f_3696, b"-0.0000033333333333333333"),
        (0x4314_3ff3_c1cb_0959, b"1424953923781206.2"),
    ];

    for &(bits, expected) in cases {
        let decoded = JcsDecoder::unlimited()
            .decode_utf8(expected)
            .expect("each RFC 8785 Appendix B decimal vector must decode");
        let actual_bits = decoded
            .as_f64()
            .expect("each RFC 8785 Appendix B vector must decode as a number")
            .to_bits();
        if expected == b"0" {
            assert_eq!(
                JcsEncoder::unlimited()
                    .to_vec(&decoded)
                    .expect("zero vector must re-encode"),
                expected,
                "positive and negative zero vectors must canonicalize to zero"
            );
        } else {
            assert_eq!(actual_bits, bits, "RFC 8785 vector {bits:016x}");
            assert_eq!(
                JcsEncoder::unlimited()
                    .to_vec(&decoded)
                    .expect("RFC 8785 vector must re-encode"),
                expected,
                "RFC 8785 vector {bits:016x} must retain its canonical decimal form"
            );
        }
    }
}

#[test]
fn test_jcs_roundtrip_nested_value_and_number_forms() {
    for input in [
        r#"{"z":[1,1.0,1e0,-0,5e-324],"a":{"x":9007199254740993}}"#,
        r#"[null,true,false,"text",{"b":2,"a":1}]"#,
    ] {
        let decoded = JcsDecoder::unlimited()
            .decode_str(input)
            .expect("each nested JSON fixture must decode");
        let canonical = JcsEncoder::unlimited()
            .to_vec(&decoded)
            .expect("each nested JSON fixture must encode canonically");
        let reparsed = JcsDecoder::unlimited()
            .decode_utf8(&canonical)
            .expect("canonical nested JSON output must decode again");
        assert_eq!(
            JcsEncoder::unlimited()
                .to_vec(&reparsed)
                .expect("reparsed nested JSON value must re-encode"),
            canonical,
            "nested values and number forms must remain stable across round trips"
        );
    }
}
