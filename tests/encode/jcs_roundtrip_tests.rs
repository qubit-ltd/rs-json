// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use qubit_json::decode::JcsDecoder;
use qubit_json::encode::JcsEncoder;

#[test]
fn test_jcs_roundtrip_reads_encoder_integer_boundary_output() {
    let first = JcsEncoder::unlimited()
        .to_vec(&serde_json::Value::from(u64::MAX))
        .unwrap();
    assert_eq!(first, b"18446744073709552000");
    let decoded = JcsDecoder::unlimited().decode_utf8(&first).unwrap();
    let second = JcsEncoder::unlimited().to_vec(&decoded).unwrap();
    assert_eq!(second, first);
}

#[test]
fn test_jcs_roundtrip_fixed_rfc_binary64_vector() {
    let value = serde_json::Value::Number(serde_json::Number::from_f64(f64::from_bits(0x4430_0000_0000_0000)).unwrap());
    let expected = b"295147905179352830000";
    let first = JcsEncoder::unlimited().to_vec(&value).unwrap();
    assert_eq!(first, expected);
    let decoded = JcsDecoder::unlimited().decode_utf8(expected).unwrap();
    assert_eq!(decoded.as_f64().unwrap().to_bits(), 0x4430_0000_0000_0000);
    assert_eq!(JcsEncoder::unlimited().to_vec(&decoded).unwrap(), expected);
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
        let decoded = JcsDecoder::unlimited().decode_utf8(expected).unwrap();
        let actual_bits = decoded.as_f64().unwrap().to_bits();
        if expected == b"0" {
            assert_eq!(JcsEncoder::unlimited().to_vec(&decoded).unwrap(), expected);
        } else {
            assert_eq!(actual_bits, bits, "RFC 8785 vector {bits:016x}");
            assert_eq!(JcsEncoder::unlimited().to_vec(&decoded).unwrap(), expected);
        }
    }
}

#[test]
fn test_jcs_roundtrip_nested_value_and_number_forms() {
    for input in [
        r#"{"z":[1,1.0,1e0,-0,5e-324],"a":{"x":9007199254740993}}"#,
        r#"[null,true,false,"text",{"b":2,"a":1}]"#,
    ] {
        let decoded = JcsDecoder::unlimited().decode_str(input).unwrap();
        let canonical = JcsEncoder::unlimited().to_vec(&decoded).unwrap();
        let reparsed = JcsDecoder::unlimited().decode_utf8(&canonical).unwrap();
        assert_eq!(JcsEncoder::unlimited().to_vec(&reparsed).unwrap(), canonical);
    }
}
