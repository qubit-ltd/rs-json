// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests the stateful strict JSON text decoder public API.

use std::error::Error;
use std::io::Cursor;
use std::io::Error as IoError;
use std::io::ErrorKind;
use std::io::Result as IoResult;

use qubit_budget::ResourceLimit;
use qubit_budget::json::JsonDecodeLimits;
use qubit_budget::json::JsonDecodeSession;
use qubit_budget::json::JsonResource;
use qubit_budget::json::JsonValueLimits;
use qubit_io::Input;
use qubit_json::decode::DiagnosticPolicy;
use qubit_json::decode::JsonDecodeErrorKind;
use qubit_json::decode::JsonDecodeErrorSource;
use qubit_json::decode::JsonDecodeStage;
use qubit_json::decode::JsonDecoder;
use qubit_json::decode::JsonSyntaxErrorReason;
use serde::Deserializer;
use serde::de::DeserializeSeed;
use serde::de::Error as DeError;
use serde_json::Value;
use serde_json::json;

struct FailingSeed;

impl<'de> DeserializeSeed<'de> for FailingSeed {
    type Value = ();

    fn deserialize<D>(self, _deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        Err(D::Error::custom("seed failure"))
    }
}

struct NonConsumingSeed;

impl<'de> DeserializeSeed<'de> for NonConsumingSeed {
    type Value = ();

    fn deserialize<D>(self, _deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(())
    }
}

/// Builds one scalar nested inside `container_depth` JSON arrays.
fn nested_array(container_depth: usize) -> String {
    format!("{}0{}", "[".repeat(container_depth), "]".repeat(container_depth))
}

/// Verifies a decoder returns a typed value for one complete document.
#[test]
fn test_json_text_decoder_decodes_typed_value() {
    let session = JsonDecodeSession::from_limits(JsonDecodeLimits::<JsonResource, usize>::builder().build());
    let value = JsonDecoder::new(session)
        .decode_utf8::<bool>(b"true")
        .expect("JSON boolean should decode");

    assert!(value);
}

/// Verifies validation accounts a complete document without deserializing it.
#[test]
fn test_json_text_decoder_validates_complete_document() {
    let session = JsonDecodeSession::from_limits(JsonDecodeLimits::<JsonResource, usize>::builder().build());
    JsonDecoder::new(session)
        .validate_utf8(br#"{"ok":[true,null]}"#)
        .expect("a complete JSON document should validate");
}

/// Verifies seed deserialization failures retain safe Serde metadata.
#[test]
fn test_json_text_decoder_maps_seed_failure() {
    let session = JsonDecodeSession::from_limits(JsonDecodeLimits::<JsonResource, usize>::builder().build());
    assert!(
        JsonDecoder::new(session)
            .decode_seed_utf8(FailingSeed, b"true")
            .is_err()
    );
}

/// Verifies a seed that leaves input unread is rejected by the final check.
#[test]
fn test_json_text_decoder_rejects_unconsumed_seed_input() {
    let session = JsonDecodeSession::from_limits(JsonDecodeLimits::<JsonResource, usize>::builder().build());
    assert!(
        JsonDecoder::new(session)
            .decode_seed_utf8(NonConsumingSeed, b"true")
            .is_err()
    );
}

/// Verifies a JSON object matching serde_json's former private number marker
/// remains an ordinary object.
#[test]
fn test_json_text_decoder_preserves_private_number_marker_object() {
    const MARKER: &str = concat!("$", "serde_json", "::private::Number");
    let input = format!(r#"{{"{MARKER}":"123"}}"#);
    let limits = JsonDecodeLimits::<JsonResource, usize>::builder()
        .value_limits(
            JsonValueLimits::builder()
                .number_bytes_limit(ResourceLimit::new(JsonResource::NumberBytes, 1))
                .build(),
        )
        .build();
    let value = JsonDecoder::with_limits(limits)
        .decode_str::<Value>(&input)
        .expect("the marker-shaped object contains no JSON number token");

    assert_eq!(value, json!({MARKER: "123"}));
}

/// Verifies integer tokens larger than `u64` are rejected before typed decode.
#[test]
fn test_json_text_decoder_rejects_integer_above_u64() {
    let error = JsonDecoder::unlimited()
        .decode_utf8::<Value>(b"18446744073709551616")
        .expect_err("an integer above u64 must be rejected");

    assert!(error.to_string().contains("outside the supported 64-bit range"));
}

/// Verifies the complete signed and unsigned 64-bit integer boundaries.
#[test]
fn test_json_text_decoder_enforces_integer_boundaries() {
    let mut decoder = JsonDecoder::unlimited();
    assert_eq!(
        decoder
            .decode_utf8::<Value>(b"-9223372036854775808")
            .expect("i64::MIN must decode"),
        json!(i64::MIN),
    );
    assert_eq!(
        decoder
            .decode_utf8::<Value>(b"18446744073709551615")
            .expect("u64::MAX must decode"),
        json!(u64::MAX),
    );
    for input in [b"-9223372036854775809".as_slice(), b"18446744073709551616".as_slice()] {
        let error = decoder
            .decode_utf8::<Value>(input)
            .expect_err("an integer outside the supported range must fail");
        assert_eq!(
            error.syntax_error().map(|error| error.reason()),
            Some(JsonSyntaxErrorReason::IntegerOutOfRange),
        );
    }
}

/// Verifies non-finite results are rejected for fractional/exponential JSON.
#[test]
fn test_json_text_decoder_rejects_float_outside_finite_f64_range() {
    let error = JsonDecoder::unlimited()
        .decode_utf8::<Value>(b"1e400")
        .expect_err("infinite f64 results must be rejected");
    assert_eq!(
        error.syntax_error().map(|error| error.reason()),
        Some(JsonSyntaxErrorReason::FloatOutOfRange),
    );
}

/// Verifies lexical number budgeting has priority over range classification.
#[test]
fn test_json_text_decoder_prioritizes_number_budget_over_range() {
    let input = b"18446744073709551616";
    let limits = JsonDecodeLimits::<JsonResource, usize>::builder()
        .value_limits(
            JsonValueLimits::builder()
                .number_bytes_limit(ResourceLimit::new(JsonResource::NumberBytes, input.len() - 1))
                .build(),
        )
        .build();
    let error = JsonDecoder::with_limits(limits)
        .decode_utf8::<Value>(input)
        .expect_err("the tighter number-byte budget must reject first");
    assert_eq!(error.kind(), JsonDecodeErrorKind::Budget);
}

/// Verifies lexical validation and target materialization expose separate
/// depth contracts.
///
/// # Panics
///
/// Panics when lexical validation inherits Serde's target-materialization
/// recursion guard, or when the guard is misclassified as lexical syntax.
#[test]
fn test_json_text_decoder_classifies_serde_depth_as_deserialization() {
    let shallow = nested_array(127);
    let deep = nested_array(128);
    let mut decoder = JsonDecoder::unlimited();

    decoder
        .validate_str(&shallow)
        .expect("the shallow document must pass lexical admission");
    decoder
        .decode_str::<Value>(&shallow)
        .expect("the shallow document must materialize as a JSON value");
    decoder
        .validate_str(&deep)
        .expect("the deep document remains valid lexical JSON");
    let error = decoder
        .decode_str::<Value>(&deep)
        .expect_err("the deep value must hit Serde's recursion guard");
    assert_eq!(error.kind(), JsonDecodeErrorKind::Deserialize);
    decoder
        .validate_utf8(deep.as_bytes())
        .expect("UTF-8 lexical admission must use the same depth contract");
    let error = decoder
        .decode_utf8::<Value>(deep.as_bytes())
        .expect_err("the deep UTF-8 value must hit Serde's recursion guard");
    assert_eq!(error.kind(), JsonDecodeErrorKind::Deserialize);
}

struct OnlyInput {
    chunks: Vec<&'static [u8]>,
    next: usize,
    interrupt_once: bool,
    fail_after_chunks: bool,
    reads: usize,
}

impl OnlyInput {
    fn new(chunks: Vec<&'static [u8]>) -> Self {
        Self {
            chunks,
            next: 0,
            interrupt_once: false,
            fail_after_chunks: false,
            reads: 0,
        }
    }
}

impl Input for OnlyInput {
    type Item = u8;

    unsafe fn read_unchecked(&mut self, output: &mut [u8], index: usize, count: usize) -> IoResult<usize> {
        self.reads += 1;
        if self.interrupt_once {
            self.interrupt_once = false;
            return Err(ErrorKind::Interrupted.into());
        }
        if self.next == self.chunks.len() {
            return if self.fail_after_chunks {
                Err(IoError::other("sensitive source text"))
            } else {
                Ok(0)
            };
        }
        let chunk = self.chunks[self.next];
        let copied = chunk.len().min(count);
        output[index..index + copied].copy_from_slice(&chunk[..copied]);
        self.next += 1;
        Ok(copied)
    }
}

#[test]
fn test_json_decoder_reads_input_only_stream_and_cursor_to_eof() {
    let mut decoder = JsonDecoder::unlimited();
    let mut input = OnlyInput::new(vec![br#"{"a":1}"#, b"  "]);
    input.interrupt_once = true;
    assert_eq!(
        decoder
            .decode_input::<Value, _>(&mut input)
            .expect("the complete input stream must decode"),
        json!({"a": 1})
    );
    assert_eq!(input.reads, 4);

    let mut cursor = Cursor::new(&b"[1,2] \n"[..]);
    assert_eq!(
        decoder
            .decode_input::<Value, _>(&mut cursor)
            .expect("the cursor must decode through trailing whitespace"),
        json!([1, 2])
    );
}

#[test]
fn test_json_decoder_stream_rejects_trailing_document() {
    let mut input = OnlyInput::new(vec![br#"{"a":1} "#, br#"{"b":2}"#]);
    let error = JsonDecoder::unlimited()
        .decode_input::<Value, _>(&mut input)
        .expect_err("a second top-level document must fail");
    assert_eq!(error.kind(), JsonDecodeErrorKind::InvalidJson);
}

#[test]
fn test_json_decoder_stream_read_errors_are_redacted_or_retained() {
    let mut input = OnlyInput::new(vec![b"true"]);
    input.fail_after_chunks = true;
    let error = JsonDecoder::unlimited()
        .decode_input::<Value, _>(&mut input)
        .expect_err("the injected read error must be reported");
    assert_eq!(error.kind(), JsonDecodeErrorKind::Read);
    assert_eq!("read".parse(), Ok(JsonDecodeErrorKind::Read));
    assert_eq!(error.stage(), JsonDecodeStage::Input);
    assert_eq!(error.raw_input_bytes(), 4);
    assert_eq!(error.read_error_kind(), Some(ErrorKind::Other));
    assert!(error.read_error().is_none());
    assert!(Error::source(&error).is_none());
    assert!(!error.to_string().contains("sensitive source text"));

    let mut input = OnlyInput::new(vec![b"true"]);
    input.fail_after_chunks = true;
    let error = JsonDecoder::unlimited()
        .with_diagnostic_policy(DiagnosticPolicy::Detailed)
        .decode_input::<Value, _>(&mut input)
        .expect_err("the injected read error must be reported");
    assert_eq!(
        error
            .read_error()
            .expect("detailed diagnostics must retain the read error")
            .to_string(),
        "sensitive source text"
    );
    let cloned = error.clone();
    assert_eq!(cloned.read_error_kind(), Some(ErrorKind::Other));
    assert!(matches!(
        error.into_source(),
        JsonDecodeErrorSource::Read { source: Some(_), .. }
    ));

    let limits = JsonDecodeLimits::<JsonResource, usize>::builder()
        .max_input_bytes(16)
        .value_limits(JsonValueLimits::builder().max_nodes(8).build())
        .build();
    let mut decoder = JsonDecoder::with_limits(limits);
    let mut input = OnlyInput::new(vec![b"true"]);
    input.fail_after_chunks = true;
    let error = decoder
        .decode_input::<Value, _>(&mut input)
        .expect_err("a mid-stream read error must abort before value admission");
    assert_eq!(error.raw_input_bytes(), 4);
    assert_eq!(
        decoder
            .session()
            .input_budget()
            .expect("input budget must be configured")
            .used(),
        4
    );
    assert_eq!(decoder.session().value_budget().used_nodes(), Some(0));

    let mut input = OnlyInput::new(Vec::new());
    input.fail_after_chunks = true;
    let error = JsonDecoder::unlimited()
        .decode_input::<Value, _>(&mut input)
        .expect_err("an error on the first read must be reported");
    assert_eq!(error.raw_input_bytes(), 0);
}

#[test]
fn test_json_decoder_stream_budget_stops_before_reading_next_chunk() {
    let limits = JsonDecodeLimits::<JsonResource, usize>::builder()
        .max_input_bytes(3)
        .build();
    let mut input = OnlyInput::new(vec![b"tr", b"ue", b"x"]);
    let error = JsonDecoder::with_limits(limits)
        .decode_input::<Value, _>(&mut input)
        .expect_err("the second chunk exceeds the cumulative input limit");
    assert_eq!(error.kind(), JsonDecodeErrorKind::Budget);
    assert_eq!(error.stage(), qubit_json::decode::JsonDecodeStage::Input);
    assert_eq!(input.next, 2);
    assert_eq!(error.raw_input_bytes(), 3);
}

#[test]
fn test_json_decoder_stream_budget_reads_only_one_probe_byte() {
    let limits = JsonDecodeLimits::<JsonResource, usize>::builder()
        .max_input_bytes(3)
        .build();
    let mut decoder = JsonDecoder::with_limits(limits);
    let mut input = Cursor::new(b"true".to_vec());
    let error = decoder
        .decode_input::<Value, _>(&mut input)
        .expect_err("the fourth byte must exceed the input budget");

    assert_eq!(error.kind(), JsonDecodeErrorKind::Budget);
    assert_eq!(error.stage(), JsonDecodeStage::Input);
    assert_eq!(input.position(), 4);
    assert_eq!(error.raw_input_bytes(), 3);
    assert_eq!(
        decoder
            .session()
            .input_budget()
            .expect("input budget must be configured")
            .used(),
        3
    );

    let limits = JsonDecodeLimits::<JsonResource, usize>::builder()
        .max_input_bytes(4)
        .build();
    let mut decoder = JsonDecoder::with_limits(limits);
    let mut input = Cursor::new(b"true".to_vec());
    assert_eq!(
        decoder
            .decode_input::<Value, _>(&mut input)
            .expect("a document ending exactly at the input limit must decode"),
        Value::Bool(true)
    );
    assert_eq!(input.position(), 4);
}

#[test]
fn test_json_decoder_stream_budget_probe_handles_zero_and_large_inputs() {
    let limits = JsonDecodeLimits::<JsonResource, usize>::builder()
        .max_input_bytes(0)
        .build();
    let mut decoder = JsonDecoder::with_limits(limits);
    let mut empty = Cursor::new(Vec::<u8>::new());
    let error = decoder
        .decode_input::<Value, _>(&mut empty)
        .expect_err("an empty stream still fails JSON syntax validation");
    assert_eq!(error.kind(), JsonDecodeErrorKind::InvalidJson);
    assert_eq!(empty.position(), 0);

    let limits = JsonDecodeLimits::<JsonResource, usize>::builder()
        .max_input_bytes(0)
        .build();
    let mut decoder = JsonDecoder::with_limits(limits);
    let mut nonempty = Cursor::new(vec![b'x'; 8192]);
    let error = decoder
        .decode_input::<Value, _>(&mut nonempty)
        .expect_err("the first byte must exceed a zero-byte budget");
    assert_eq!(error.kind(), JsonDecodeErrorKind::Budget);
    assert_eq!(nonempty.position(), 1);
    assert_eq!(error.raw_input_bytes(), 0);

    let limits = JsonDecodeLimits::<JsonResource, usize>::builder()
        .max_input_bytes(2)
        .build();
    let mut decoder = JsonDecoder::with_limits(limits);
    let mut large = Cursor::new(vec![b'x'; 8192]);
    let error = decoder
        .decode_input::<Value, _>(&mut large)
        .expect_err("the probe byte beyond the two-byte limit must reject the stream");
    assert_eq!(error.kind(), JsonDecodeErrorKind::Budget);
    assert_eq!(large.position(), 3);
    assert_eq!(error.raw_input_bytes(), 2);
}

#[test]
fn test_json_decoder_stream_budget_handles_chunk_sized_allowances() {
    let mut exact_chunk = vec![b'x'; 8190];
    exact_chunk.insert(0, b'"');
    exact_chunk.push(b'"');
    let limits = JsonDecodeLimits::<JsonResource, usize>::builder()
        .max_input_bytes(8192)
        .build();
    let mut decoder = JsonDecoder::with_limits(limits);
    let mut input = Cursor::new(exact_chunk);
    let value = decoder
        .decode_input::<String, _>(&mut input)
        .expect("a complete document exactly one chunk long must decode");
    assert_eq!(value.len(), 8190);
    assert_eq!(input.position(), 8192);

    let mut multiple_chunks = vec![b'x'; 16382];
    multiple_chunks.insert(0, b'"');
    multiple_chunks.push(b'"');
    let limits = JsonDecodeLimits::<JsonResource, usize>::builder()
        .max_input_bytes(16385)
        .build();
    let mut decoder = JsonDecoder::with_limits(limits);
    let mut input = Cursor::new(multiple_chunks);
    let value = decoder
        .decode_input::<String, _>(&mut input)
        .expect("a document spanning chunks with more than one chunk remaining must decode");
    assert_eq!(value.len(), 16382);
    assert_eq!(input.position(), 16384);
}

#[test]
fn test_json_decoder_stream_budget_supports_small_quantity_types() {
    let limits = JsonDecodeLimits::<JsonResource, u8>::builder()
        .max_input_bytes(3)
        .build();
    let mut decoder = JsonDecoder::with_limits(limits);
    let mut input = Cursor::new(b"true".to_vec());
    let error = decoder
        .decode_input::<Value, _>(&mut input)
        .expect_err("the u8 budget must reject the fourth input byte");

    assert_eq!(error.kind(), JsonDecodeErrorKind::Budget);
    assert_eq!(input.position(), 4);
    assert_eq!(error.raw_input_bytes(), 3);
    assert_eq!(
        decoder
            .session()
            .input_budget()
            .expect("input budget must be configured")
            .used(),
        3
    );

    let mut max_u8_document = vec![b'x'; 253];
    max_u8_document.insert(0, b'"');
    max_u8_document.push(b'"');
    let limits = JsonDecodeLimits::<JsonResource, u8>::builder()
        .max_input_bytes(u8::MAX)
        .build();
    let mut decoder = JsonDecoder::with_limits(limits);
    let mut input = Cursor::new(max_u8_document);
    let value = decoder
        .decode_input::<String, _>(&mut input)
        .expect("a document exactly at the largest u8 budget must decode");
    assert_eq!(value.len(), 253);
    assert_eq!(input.position(), u64::from(u8::MAX));
}

#[test]
fn test_json_decoder_stream_classifies_empty_input_and_invalid_utf8() {
    let mut empty = OnlyInput::new(Vec::new());
    let error = JsonDecoder::unlimited()
        .decode_input::<Value, _>(&mut empty)
        .expect_err("an empty stream must fail strict JSON decoding");
    assert_eq!(error.kind(), JsonDecodeErrorKind::InvalidJson);
    assert_eq!(error.raw_input_bytes(), 0);

    let mut invalid_utf8 = OnlyInput::new(vec![b"\xff"]);
    let error = JsonDecoder::unlimited()
        .decode_input::<Value, _>(&mut invalid_utf8)
        .expect_err("invalid UTF-8 must fail before JSON parsing");
    assert_eq!(error.kind(), JsonDecodeErrorKind::InvalidUtf8);
    assert_eq!(error.raw_input_bytes(), 1);
}

#[test]
fn test_json_decoder_stream_cumulative_input_budget_spans_calls() {
    let limits = JsonDecodeLimits::<JsonResource, usize>::builder()
        .max_input_bytes(5)
        .value_limits(JsonValueLimits::builder().max_nodes(8).build())
        .build();
    let mut decoder = JsonDecoder::with_limits(limits);
    let mut first = OnlyInput::new(vec![b"true"]);
    assert_eq!(
        decoder
            .decode_input::<Value, _>(&mut first)
            .expect("the first stream must fit the cumulative input budget"),
        Value::Bool(true)
    );

    let mut second = OnlyInput::new(vec![b"false"]);
    let error = decoder
        .decode_input::<Value, _>(&mut second)
        .expect_err("the second call must use the remaining cumulative input budget");
    assert_eq!(error.kind(), JsonDecodeErrorKind::Budget);
    assert_eq!(error.raw_input_bytes(), 1);
    assert_eq!(second.next, 1);
    assert_eq!(
        decoder
            .session()
            .input_budget()
            .expect("input budget must be configured")
            .used(),
        5
    );
    assert_eq!(decoder.session().value_budget().used_nodes(), Some(1));
}

#[test]
fn test_json_decoder_stream_accounts_bytes_once_and_does_not_commit_failures() {
    let limits = JsonDecodeLimits::<JsonResource, usize>::builder()
        .max_input_bytes(16)
        .value_limits(JsonValueLimits::builder().max_nodes(10).build())
        .build();
    let mut decoder = JsonDecoder::with_limits(limits);
    let mut input = OnlyInput::new(vec![b"true"]);
    assert_eq!(
        decoder
            .decode_input::<Value, _>(&mut input)
            .expect("the complete stream must decode"),
        Value::Bool(true)
    );
    assert_eq!(
        decoder
            .session()
            .input_budget()
            .expect("input budget must be configured")
            .used(),
        4
    );

    let mut invalid = OnlyInput::new(vec![b"{"]);
    assert!(decoder.decode_input::<Value, _>(&mut invalid).is_err());
    assert_eq!(
        decoder
            .session()
            .input_budget()
            .expect("input budget must be configured")
            .used(),
        5
    );
    assert_eq!(decoder.session().value_budget().used_nodes(), Some(1));
}
