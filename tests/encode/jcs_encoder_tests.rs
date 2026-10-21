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

use qubit_budget::json::JsonEncodeLimits;
use qubit_budget::json::JsonEncodeSession;
use qubit_budget::json::JsonResource;
use qubit_json::encode::JcsEncodeErrorKind;
use qubit_json::encode::JcsEncoder;

#[derive(Clone)]
struct SharedOutput(Rc<RefCell<Vec<u8>>>);

impl qubit_io::Output for SharedOutput {
    type Item = u8;

    unsafe fn write_unchecked(&mut self, input: &[u8], index: usize, count: usize) -> io::Result<usize> {
        self.0.borrow_mut().extend_from_slice(&input[index..index + count]);
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

impl qubit_io::Output for PrefixOutput {
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
fn test_jcs_encoder_exercises_construction_and_session_methods() {
    let limits = JsonEncodeLimits::<JsonResource, usize>::builder()
        .max_output_bytes(16)
        .max_nodes(4)
        .build();
    let mut encoder = JcsEncoder::new(JsonEncodeSession::from_limits(limits));
    assert_eq!(encoder.session().max_output_bytes(), Some(16));
    assert_eq!(encoder.session_mut().max_output_bytes(), Some(16));
    assert_eq!(encoder.to_vec(&serde_json::Value::Bool(true)).unwrap(), b"true");
    assert_eq!(encoder.session().output_budget().unwrap().used(), 4);
    let session = encoder.into_session();
    assert_eq!(session.output_budget().unwrap().used(), 4);

    let configured = JcsEncoder::with_limits(JsonEncodeLimits::<JsonResource, usize>::new());
    assert_eq!(configured.session().max_output_bytes(), None);
    assert_eq!(JcsEncoder::unlimited().session().max_output_bytes(), None);
}

#[test]
fn test_jcs_encoder_output_methods_return_expected_bytes() {
    let value = serde_json::json!({"b": 2, "a": 1.0});
    let mut encoder = JcsEncoder::unlimited();
    assert_eq!(encoder.to_string(&value).unwrap(), r#"{"a":1,"b":2}"#);

    let buffered = Rc::new(RefCell::new(Vec::new()));
    encoder.write_buffered(SharedOutput(buffered.clone()), &value).unwrap();
    assert_eq!(*buffered.borrow(), br#"{"a":1,"b":2}"#);

    let incremental = Rc::new(RefCell::new(Vec::new()));
    encoder
        .write_incremental(SharedOutput(incremental.clone()), &value)
        .unwrap();
    assert_eq!(*incremental.borrow(), br#"{"a":1,"b":2}"#);
}

#[test]
fn test_jcs_encoder_value_and_output_budget_boundaries_are_transactional() {
    let limits = JsonEncodeLimits::<JsonResource, usize>::builder()
        .max_nodes(1)
        .max_output_bytes(16)
        .build();
    let mut encoder = JcsEncoder::with_limits(limits);
    let error = encoder.to_vec(&serde_json::json!([null])).unwrap_err();
    assert_eq!(error.kind(), JcsEncodeErrorKind::Budget);
    assert_eq!(encoder.session().value_budget().used_nodes(), Some(0));

    let exact = JsonEncodeLimits::<JsonResource, usize>::builder()
        .max_output_bytes(7)
        .build();
    let mut encoder = JcsEncoder::with_limits(exact);
    assert_eq!(encoder.to_vec(&serde_json::json!({"a": 1})).unwrap(), br#"{"a":1}"#);

    let too_small = JsonEncodeLimits::<JsonResource, usize>::builder()
        .max_output_bytes(6)
        .build();
    let mut encoder = JcsEncoder::with_limits(too_small);
    assert_eq!(
        encoder.to_vec(&serde_json::json!({"a": 1})).unwrap_err().kind(),
        JcsEncodeErrorKind::Budget
    );
    assert_eq!(encoder.session().output_budget().unwrap().used(), 0);
}

#[test]
fn test_jcs_encoder_buffered_write_error_keeps_only_accepted_prefix() {
    let (output, bytes) = output_bytes(3);
    let limits = JsonEncodeLimits::<JsonResource, usize>::builder()
        .max_output_bytes(16)
        .max_nodes(8)
        .build();
    let mut encoder = JcsEncoder::with_limits(limits);
    let error = encoder
        .write_buffered(output, &serde_json::json!({"a": 1}))
        .unwrap_err();
    assert_eq!(error.kind(), JcsEncodeErrorKind::Write);
    assert_eq!(*bytes.borrow(), br#"{"a"#);
    assert_eq!(encoder.session().output_budget().unwrap().used(), 3);
    assert_eq!(encoder.session().value_budget().used_nodes(), Some(0));
}

#[test]
fn test_jcs_encoder_incremental_write_error_keeps_only_accepted_prefix() {
    let (output, bytes) = output_bytes(3);
    let limits = JsonEncodeLimits::<JsonResource, usize>::builder()
        .max_output_bytes(16)
        .max_nodes(8)
        .build();
    let mut encoder = JcsEncoder::with_limits(limits);
    let error = encoder
        .write_incremental(output, &serde_json::json!({"a": 1}))
        .unwrap_err();
    assert_eq!(error.kind(), JcsEncodeErrorKind::Write);
    assert_eq!(*bytes.borrow(), br#"{"a"#);
    assert_eq!(encoder.session().output_budget().unwrap().used(), 3);
    assert_eq!(encoder.session().value_budget().used_nodes(), Some(0));
}
