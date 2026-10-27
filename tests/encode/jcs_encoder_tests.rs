// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Verifies the public JSON Canonicalization Scheme encoder contract.

use std::cell::RefCell;
use std::io;
use std::rc::Rc;
use std::str::FromStr;

use qubit_budget::json::JsonEncodeLimits;
use qubit_budget::json::JsonEncodeSession;
use qubit_budget::json::JsonResource;
use qubit_io::Output;
use qubit_json::encode::JcsEncodeErrorKind;
use qubit_json::encode::JcsEncoder;

#[derive(Clone)]
struct SharedOutput(Rc<RefCell<Vec<u8>>>);

impl Output for SharedOutput {
    type Item = u8;

    unsafe fn write_unchecked(&mut self, input: &[u8], index: usize, count: usize) -> io::Result<usize> {
        self.0.borrow_mut().extend_from_slice(&input[index..index + count]);
        Ok(count)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct OutputOnly(Rc<RefCell<Vec<u8>>>);

impl Output for OutputOnly {
    type Item = u8;

    unsafe fn write_unchecked(&mut self, bytes: &[u8], index: usize, count: usize) -> io::Result<usize> {
        self.0.borrow_mut().extend_from_slice(&bytes[index..index + count]);
        Ok(count)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct PrefixOutput {
    bytes: Rc<RefCell<Vec<u8>>>,
    max_bytes: usize,
}

impl Output for PrefixOutput {
    type Item = u8;

    unsafe fn write_unchecked(&mut self, input: &[u8], index: usize, count: usize) -> io::Result<usize> {
        let mut bytes = self.bytes.borrow_mut();
        let remaining = self.max_bytes.saturating_sub(bytes.len());
        if remaining == 0 {
            return Err(io::Error::other("injected destination failure"));
        }
        let written = count.min(remaining);
        bytes.extend_from_slice(&input[index..index + written]);
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct FlushFailureOutput(Rc<RefCell<Vec<u8>>>);

impl Output for FlushFailureOutput {
    type Item = u8;

    unsafe fn write_unchecked(&mut self, input: &[u8], index: usize, count: usize) -> io::Result<usize> {
        self.0.borrow_mut().extend_from_slice(&input[index..index + count]);
        Ok(count)
    }

    fn flush(&mut self) -> io::Result<()> {
        Err(io::Error::other("injected flush failure"))
    }
}

struct ZeroOutput;

impl Output for ZeroOutput {
    type Item = u8;

    unsafe fn write_unchecked(&mut self, _input: &[u8], _index: usize, _count: usize) -> io::Result<usize> {
        Ok(0)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn output_bytes(limit: usize) -> (PrefixOutput, Rc<RefCell<Vec<u8>>>) {
    let bytes = Rc::new(RefCell::new(Vec::new()));
    (
        PrefixOutput {
            bytes: bytes.clone(),
            max_bytes: limit,
        },
        bytes,
    )
}

#[test]
fn test_jcs_encoder_sorts_objects_recursively_and_uses_utf16_key_order() {
    let mut encoder = JcsEncoder::unlimited();
    let value = serde_json::json!({
        "outer": [{"b": 2, "a": 1}],
        "דּ": 2,
        "😀": 1,
    });
    assert_eq!(
        encoder.to_vec(&value).unwrap(),
        r#"{"outer":[{"a":1,"b":2}],"😀":1,"דּ":2}"#.as_bytes(),
    );
}

#[test]
fn test_jcs_encoder_emits_rfc_number_and_escape_bytes() {
    let mut encoder = JcsEncoder::unlimited();
    let escaped = serde_json::json!({"x": "\0\t\n\\\""});
    assert_eq!(encoder.to_vec(&escaped).unwrap(), br#"{"x":"\u0000\t\n\\\""}"#);

    let negative_zero = serde_json::Value::Number(serde_json::Number::from_f64(-0.0).unwrap());
    assert_eq!(encoder.to_vec(&negative_zero).unwrap(), b"0");
    assert_eq!(encoder.to_vec(&serde_json::json!(1e30_f64)).unwrap(), b"1e+30");
}

#[test]
fn test_jcs_encoder_rounds_binary64_integer_boundaries_as_specified() {
    let cases = [
        (9_007_199_254_740_991_u64, b"9007199254740991".as_slice()),
        (9_007_199_254_740_992_u64, b"9007199254740992".as_slice()),
        (9_007_199_254_740_993_u64, b"9007199254740992".as_slice()),
        (u64::MAX, b"18446744073709552000".as_slice()),
    ];
    let mut encoder = JcsEncoder::unlimited();
    for (number, expected) in cases {
        assert_eq!(encoder.to_vec(&number.into()).unwrap(), expected);
    }
}

#[test]
fn test_jcs_encoder_matches_rfc8785_appendix_b_vectors() {
    // RFC 8785 Appendix B, Table 1. Expected strings are fixed RFC vectors.
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
        let number = serde_json::Number::from_f64(f64::from_bits(bits))
            .expect("RFC finite binary64 vector is representable as a JSON number");
        let value = serde_json::Value::Number(number);
        assert_eq!(
            JcsEncoder::unlimited().to_vec(&value).unwrap(),
            expected,
            "RFC 8785 Appendix B vector {bits:016x}",
        );
    }

    assert!(serde_json::Number::from_f64(f64::NAN).is_none());
    assert!(serde_json::Number::from_f64(f64::INFINITY).is_none());
    assert!(serde_json::Number::from_f64(f64::NEG_INFINITY).is_none());
}

#[test]
fn test_jcs_preserves_unicode_forms_and_budget_independent_bytes() {
    let value = serde_json::json!({"a": "é", "b": "e\u{301}"});
    let expected = JcsEncoder::unlimited().to_vec(&value).unwrap();
    assert_eq!(expected, "{\"a\":\"é\",\"b\":\"e\u{301}\"}".as_bytes());

    for maximum in [expected.len(), expected.len() + 64] {
        let limits = JsonEncodeLimits::<JsonResource, usize>::builder()
            .max_output_bytes(maximum)
            .build();
        let actual = JcsEncoder::with_limits(limits, 1024 * 1024).to_vec(&value).unwrap();
        assert_eq!(actual, expected);
    }

    let limits = JsonEncodeLimits::<JsonResource, usize>::builder()
        .max_output_bytes(expected.len() - 1)
        .build();
    assert!(JcsEncoder::with_limits(limits, 1024 * 1024).to_vec(&value).is_err());
}

#[test]
fn test_jcs_encoder_exercises_construction_and_session_methods() {
    let limits = JsonEncodeLimits::<JsonResource, usize>::builder()
        .max_output_bytes(16)
        .max_nodes(4)
        .build();
    let mut encoder = JcsEncoder::new(JsonEncodeSession::from_limits(limits), 4096);
    assert_eq!(encoder.max_working_bytes(), 4096);
    assert_eq!(encoder.session().max_output_bytes(), Some(16));
    assert_eq!(encoder.session_mut().max_output_bytes(), Some(16));
    assert_eq!(encoder.to_vec(&serde_json::Value::Bool(true)).unwrap(), b"true");
    assert_eq!(encoder.session().output_budget().unwrap().used(), 4);
    let session = encoder.into_session();
    assert_eq!(session.output_budget().unwrap().used(), 4);

    let configured = JcsEncoder::with_limits(JsonEncodeLimits::<JsonResource, usize>::new(), 8192);
    assert_eq!(configured.session().max_output_bytes(), None);
    assert_eq!(configured.max_working_bytes(), 8192);
    assert_eq!(JcsEncoder::unlimited().session().max_output_bytes(), None);
}

#[test]
fn test_jcs_encode_error_kind_parses_and_displays_working_memory() {
    let kind = JcsEncodeErrorKind::from_str("working_memory").unwrap();
    assert_eq!(kind, JcsEncodeErrorKind::WorkingMemory);
    assert_eq!(kind.to_string(), "working_memory");
    assert_eq!(JcsEncodeErrorKind::from_str("WORKING_MEMORY").unwrap(), kind);
}

#[test]
fn test_jcs_encoder_output_methods_return_expected_bytes() {
    let value = serde_json::json!({"b": 2, "a": 1.0});
    let mut encoder = JcsEncoder::unlimited();
    assert_eq!(encoder.to_string(&value).unwrap(), r#"{"a":1,"b":2}"#);

    let buffered = Rc::new(RefCell::new(Vec::new()));
    encoder
        .write_buffered(&mut SharedOutput(buffered.clone()), &value)
        .unwrap();
    assert_eq!(*buffered.borrow(), br#"{"a":1,"b":2}"#);

    let incremental = Rc::new(RefCell::new(Vec::new()));
    encoder
        .write_incremental(&mut SharedOutput(incremental.clone()), &value)
        .unwrap();
    assert_eq!(*incremental.borrow(), br#"{"a":1,"b":2}"#);
}

#[test]
fn test_jcs_encoder_canonicalizes_object() {
    let mut encoder = JcsEncoder::unlimited();
    let value = serde_json::json!({"b": 2, "a": 1.0});
    assert_eq!(encoder.to_vec(&value).unwrap(), br#"{"a":1,"b":2}"#);
}

#[test]
fn test_jcs_encoder_writes_to_output_only_destination() {
    let value = serde_json::json!({"b": 2, "a": 1});
    let mut buffered_encoder = JcsEncoder::unlimited();
    let buffered = Rc::new(RefCell::new(Vec::new()));
    buffered_encoder
        .write_buffered(&mut OutputOnly(buffered.clone()), &value)
        .unwrap();
    assert_eq!(*buffered.borrow(), br#"{"a":1,"b":2}"#);

    let mut incremental_encoder = JcsEncoder::unlimited();
    let incremental = Rc::new(RefCell::new(Vec::new()));
    incremental_encoder
        .write_incremental(&mut OutputOnly(incremental.clone()), &value)
        .unwrap();
    assert_eq!(*incremental.borrow(), br#"{"a":1,"b":2}"#);
}

#[test]
fn test_jcs_encoder_value_and_output_budget_boundaries_are_transactional() {
    let limits = JsonEncodeLimits::<JsonResource, usize>::builder()
        .max_nodes(1)
        .max_output_bytes(16)
        .build();
    let mut encoder = JcsEncoder::with_limits(limits, 4096);
    let error = encoder.to_vec(&serde_json::json!([null])).unwrap_err();
    assert_eq!(error.kind(), JcsEncodeErrorKind::Budget);
    assert!(error.budget_error().is_some());
    assert!(error.working_memory_error().is_none());
    assert_eq!(encoder.session().value_budget().used_nodes(), Some(0));

    let exact = JsonEncodeLimits::<JsonResource, usize>::builder()
        .max_output_bytes(7)
        .build();
    let mut encoder = JcsEncoder::with_limits(exact, 4096);
    assert_eq!(encoder.to_vec(&serde_json::json!({"a": 1})).unwrap(), br#"{"a":1}"#);

    let too_small = JsonEncodeLimits::<JsonResource, usize>::builder()
        .max_output_bytes(6)
        .build();
    let mut encoder = JcsEncoder::with_limits(too_small, 4096);
    assert_eq!(
        encoder.to_vec(&serde_json::json!({"a": 1})).unwrap_err().kind(),
        JcsEncodeErrorKind::Budget
    );
    assert_eq!(encoder.session().output_budget().unwrap().used(), 0);
}

#[test]
fn test_jcs_encoder_buffered_write_error_keeps_only_accepted_prefix() {
    let (mut output, bytes) = output_bytes(3);
    let limits = JsonEncodeLimits::<JsonResource, usize>::builder()
        .max_output_bytes(16)
        .max_nodes(8)
        .build();
    let mut encoder = JcsEncoder::with_limits(limits, 4096);
    let error = encoder
        .write_buffered(&mut output, &serde_json::json!({"a": 1}))
        .unwrap_err();
    assert_eq!(error.kind(), JcsEncodeErrorKind::Write);
    assert!(error.write_error().is_some());
    assert!(error.budget_error().is_none());
    assert_eq!(*bytes.borrow(), br#"{"a"#);
    assert_eq!(encoder.session().output_budget().unwrap().used(), 3);
    assert_eq!(encoder.session().value_budget().used_nodes(), Some(0));
}

#[test]
fn test_jcs_encoder_incremental_write_error_keeps_only_accepted_prefix() {
    let (mut output, bytes) = output_bytes(3);
    let limits = JsonEncodeLimits::<JsonResource, usize>::builder()
        .max_output_bytes(16)
        .max_nodes(8)
        .build();
    let mut encoder = JcsEncoder::with_limits(limits, 4096);
    let error = encoder
        .write_incremental(&mut output, &serde_json::json!({"a": 1}))
        .unwrap_err();
    assert_eq!(error.kind(), JcsEncodeErrorKind::Write);
    assert!(error.write_error().is_some());
    assert!(error.budget_error().is_none());
    assert_eq!(*bytes.borrow(), br#"{"a"#);
    assert_eq!(encoder.session().output_budget().unwrap().used(), 3);
    assert_eq!(encoder.session().value_budget().used_nodes(), Some(0));
}

#[test]
fn test_jcs_encoder_rejects_object_before_working_memory_exceeds_limit() {
    let value = serde_json::json!({"a": 1, "b": 2});
    let limits = JsonEncodeLimits::<JsonResource, usize>::builder()
        .max_nodes(8)
        .max_output_bytes(32)
        .build();
    let mut encoder = JcsEncoder::with_limits(limits, 0);
    let error = encoder.to_vec(&value).unwrap_err();
    assert_eq!(error.kind(), JcsEncodeErrorKind::WorkingMemory);
    let memory = error.working_memory_error().unwrap();
    assert_eq!(memory.limit(), 0);
    assert!(memory.requested() > 0);
    assert_eq!(
        error.to_string(),
        format!(
            "JCS working memory limit exceeded (limit: 0, requested: {})",
            memory.requested(),
        ),
    );
    assert_eq!(
        std::error::Error::source(&error).unwrap().to_string(),
        error.to_string(),
    );
    assert!(error.budget_error().is_none());
    assert!(error.serialization_error().is_none());
    assert!(error.write_error().is_none());
    assert_eq!(encoder.session().value_budget().used_nodes(), Some(0));
    assert_eq!(encoder.session().output_budget().unwrap().used(), 0);
    assert_eq!(encoder.to_vec(&serde_json::json!(true)).unwrap(), b"true");
    assert_eq!(encoder.session().value_budget().used_nodes(), Some(1));
}

#[test]
fn test_jcs_encoder_admission_depth_precedes_renderer_working_memory() {
    let mut value = serde_json::Value::Null;
    for _ in 0..64 {
        value = serde_json::Value::Array(vec![value]);
    }

    let loose_limits = JsonEncodeLimits::<JsonResource, usize>::builder()
        .max_nodes(128)
        .max_output_bytes(4096)
        .build();
    let error = JcsEncoder::with_limits(loose_limits, 0)
        .to_vec(&value)
        .expect_err("renderer stack must be rejected at zero working capacity");
    assert_eq!(error.kind(), JcsEncodeErrorKind::WorkingMemory);

    let shallow_limits = JsonEncodeLimits::<JsonResource, usize>::builder()
        .max_depth(32)
        .max_nodes(128)
        .max_output_bytes(4096)
        .build();
    let error = JcsEncoder::with_limits(shallow_limits, 0)
        .to_vec(&value)
        .expect_err("admission depth must fail before renderer capacity is checked");
    assert_eq!(error.kind(), JcsEncodeErrorKind::Budget);
    assert!(error.budget_error().is_some());

    while let serde_json::Value::Array(mut values) = value {
        value = values.pop().expect("each nested array has one child");
    }
}

#[test]
fn test_jcs_encoder_checks_working_memory_for_nested_containers_and_reuses_session() {
    let value = serde_json::json!([{"a": [1, 2]}]);
    let limits = JsonEncodeLimits::<JsonResource, usize>::builder().max_nodes(8).build();
    let mut encoder = JcsEncoder::with_limits(limits, 1);
    assert_eq!(
        encoder.to_vec(&value).unwrap_err().kind(),
        JcsEncodeErrorKind::WorkingMemory
    );
    assert_eq!(encoder.session().value_budget().used_nodes(), Some(0));

    let sufficient_limits = JsonEncodeLimits::<JsonResource, usize>::builder().max_nodes(8).build();
    let mut sufficient = JcsEncoder::with_limits(sufficient_limits, 4096);
    assert_eq!(sufficient.to_vec(&value).unwrap(), br#"[{"a":[1,2]}]"#);
    assert_eq!(sufficient.session().value_budget().used_nodes(), Some(5));
}

#[test]
fn test_jcs_encoder_working_memory_exact_boundary_for_object() {
    let value = serde_json::json!({"a": 1, "b": 2});
    let mut exact_limit = 0;
    loop {
        let limits = JsonEncodeLimits::<JsonResource, usize>::builder().max_nodes(8).build();
        let mut encoder = JcsEncoder::with_limits(limits, exact_limit);
        match encoder.to_vec(&value) {
            Ok(bytes) => {
                assert_eq!(bytes, br#"{"a":1,"b":2}"#);
                assert_eq!(encoder.session().value_budget().used_nodes(), Some(3));
                break;
            }
            Err(error) => {
                assert_eq!(error.kind(), JcsEncodeErrorKind::WorkingMemory);
                exact_limit = error.working_memory_error().unwrap().requested();
            }
        }
        assert!(exact_limit < 4096, "working-memory boundary did not converge");
    }

    let limits = JsonEncodeLimits::<JsonResource, usize>::builder().max_nodes(8).build();
    let mut one_byte_short = JcsEncoder::with_limits(limits, exact_limit - 1);
    let error = one_byte_short.to_vec(&value).unwrap_err();
    assert_eq!(error.kind(), JcsEncodeErrorKind::WorkingMemory);
    assert_eq!(one_byte_short.session().value_budget().used_nodes(), Some(0));
}

#[test]
fn test_jcs_encoder_zero_byte_destination_is_write_zero_and_rolls_back_value() {
    let limits = JsonEncodeLimits::<JsonResource, usize>::builder()
        .max_nodes(8)
        .max_output_bytes(16)
        .build();
    let mut encoder = JcsEncoder::with_limits(limits, 4096);
    let error = encoder
        .write_incremental(&mut ZeroOutput, &serde_json::json!({"a": 1}))
        .unwrap_err();
    assert_eq!(error.kind(), JcsEncodeErrorKind::Write);
    assert_eq!(error.write_error().unwrap().kind(), io::ErrorKind::WriteZero);
    assert_eq!(encoder.session().output_budget().unwrap().used(), 0);
    assert_eq!(encoder.session().value_budget().used_nodes(), Some(0));
}

#[test]
fn test_jcs_encoder_small_output_budget_fails_with_sufficient_working_memory() {
    let limits = JsonEncodeLimits::<JsonResource, usize>::builder()
        .max_output_bytes(1)
        .max_nodes(8)
        .build();
    let mut encoder = JcsEncoder::with_limits(limits, 4096);
    let error = encoder.to_vec(&serde_json::json!({"large": "value"})).unwrap_err();
    assert_eq!(error.kind(), JcsEncodeErrorKind::Budget);
    assert_eq!(encoder.session().value_budget().used_nodes(), Some(0));
    assert_eq!(encoder.session().output_budget().unwrap().used(), 0);
}

#[test]
fn test_jcs_encoder_large_object_with_tiny_output_limit_is_transactional() {
    let mut object = serde_json::Map::new();
    for index in 0..128 {
        object.insert(format!("key-{index:03}"), serde_json::json!(index));
    }
    let value = serde_json::Value::Object(object);
    let limits = JsonEncodeLimits::<JsonResource, usize>::builder()
        .max_output_bytes(1)
        .max_nodes(256)
        .build();
    let mut encoder = JcsEncoder::with_limits(limits, 16 * 1024);
    let error = encoder.to_vec(&value).unwrap_err();
    assert_eq!(error.kind(), JcsEncodeErrorKind::Budget);
    assert_eq!(encoder.session().value_budget().used_nodes(), Some(0));
    assert_eq!(encoder.session().output_budget().unwrap().used(), 0);
}

#[test]
fn test_jcs_encoder_scalar_needs_no_working_memory() {
    let limits = JsonEncodeLimits::<JsonResource, usize>::builder().max_nodes(1).build();
    let mut encoder = JcsEncoder::with_limits(limits, 0);
    assert_eq!(
        encoder.to_vec(&serde_json::json!("a large scalar string")).unwrap(),
        br#""a large scalar string""#
    );
}

#[test]
fn test_jcs_encoder_all_output_paths_reject_insufficient_working_memory() {
    let value = serde_json::json!({"a": 1});

    let limits = JsonEncodeLimits::<JsonResource, usize>::builder().max_nodes(4).build();
    let mut string_encoder = JcsEncoder::with_limits(limits, 0);
    assert_eq!(
        string_encoder.to_string(&value).unwrap_err().kind(),
        JcsEncodeErrorKind::WorkingMemory
    );

    let limits = JsonEncodeLimits::<JsonResource, usize>::builder().max_nodes(4).build();
    let mut buffered_encoder = JcsEncoder::with_limits(limits, 0);
    let buffered = Rc::new(RefCell::new(Vec::new()));
    assert_eq!(
        buffered_encoder
            .write_buffered(&mut SharedOutput(buffered.clone()), &value)
            .unwrap_err()
            .kind(),
        JcsEncodeErrorKind::WorkingMemory,
    );
    assert!(buffered.borrow().is_empty());

    let limits = JsonEncodeLimits::<JsonResource, usize>::builder().max_nodes(4).build();
    let mut incremental_encoder = JcsEncoder::with_limits(limits, 0);
    let incremental = Rc::new(RefCell::new(Vec::new()));
    assert_eq!(
        incremental_encoder
            .write_incremental(&mut SharedOutput(incremental.clone()), &value)
            .unwrap_err()
            .kind(),
        JcsEncodeErrorKind::WorkingMemory,
    );
    assert!(incremental.borrow().is_empty());
}

#[test]
fn test_jcs_encoder_incremental_flush_failure_keeps_output_and_rolls_back_value() {
    let bytes = Rc::new(RefCell::new(Vec::new()));
    let limits = JsonEncodeLimits::<JsonResource, usize>::builder()
        .max_nodes(8)
        .max_output_bytes(32)
        .build();
    let mut encoder = JcsEncoder::with_limits(limits, 4096);
    let error = encoder
        .write_incremental(&mut FlushFailureOutput(bytes.clone()), &serde_json::json!({"a": 1}))
        .unwrap_err();
    assert_eq!(error.kind(), JcsEncodeErrorKind::Write);
    assert_eq!(*bytes.borrow(), br#"{"a":1}"#);
    assert_eq!(encoder.session().value_budget().used_nodes(), Some(0));
    assert_eq!(encoder.session().output_budget().unwrap().used(), 7);
}

use qubit_io::ByteOutput;

#[cfg(feature = "jcs")]
#[test]
fn test_jcs_encoder_accepts_dyn_byte_output() {
    let mut bytes = Vec::new();
    let output: &mut dyn ByteOutput = &mut bytes;
    qubit_json::encode::JcsEncoder::unlimited()
        .write_buffered(output, &serde_json::json!(true))
        .unwrap();
    assert_eq!(bytes, b"true");
}
