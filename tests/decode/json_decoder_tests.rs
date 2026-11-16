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
use qubit_json::decode::JsonNumberPolicy;
use qubit_json::decode::JsonPathSegment;
use qubit_json::decode::JsonSyntaxErrorReason;
use qubit_json::decode::JsonValidatedDecodeOptions;
use qubit_json::value::DuplicateKeyRejectingJsonValue;
use serde::Deserializer;
use serde::de::DeserializeSeed;
use serde::de::Error as DeError;
use serde_json::Error as SerdeJsonError;
use serde_json::Value;
use serde_json::json;

#[test]
fn test_indexed_object_keys_preserve_decoded_identity() {
    let duplicate = r#"{"a":1,"\u0061":2}"#;
    let error = JsonDecoder::unlimited().decode_value_str(duplicate).unwrap_err();
    assert_eq!(error.kind(), JsonDecodeErrorKind::Deserialize);
    assert_eq!(error.line(), Some(1));
    assert_eq!(error.column(), Some(8));

    let error = JsonDecoder::unlimited()
        .decode_unique_str::<DuplicateKeyRejectingJsonValue>(duplicate)
        .unwrap_err();
    assert_eq!(error.kind(), JsonDecodeErrorKind::Deserialize);
    assert_eq!(error.column(), Some(8));

    let nested = JsonDecoder::unlimited()
        .decode_value_str(r#"{"a":1,"child":{"a":2}}"#)
        .expect("equal keys in separate objects are valid");
    assert_eq!(nested.as_value(), &json!({"a": 1, "child": {"a": 2}}));
    let options = JsonValidatedDecodeOptions::builder().build();
    let validated = JsonDecoder::unlimited()
        .decode_unique_validated_str(r#"{"a":1,"child":{"a":2}}"#, &options, |_, _| {
            Ok::<_, std::convert::Infallible>(JsonNumberPolicy::Preserve)
        })
        .expect("validated preserve must accept the same document");
    assert_eq!(nested.as_value(), validated.as_value());

    let limits = JsonDecodeLimits::builder()
        .input_bytes_limit(ResourceLimit::new(
            qubit_budget::json::JsonResource::InputBytes,
            4_usize,
        ))
        .build();
    let mut decoder = JsonDecoder::with_limits(limits);
    let mut input = Cursor::new(b"true".to_vec());
    assert_eq!(
        decoder
            .decode_value_input(&mut input)
            .expect("owned input path must not charge bytes twice")
            .as_value(),
        &Value::Bool(true)
    );
    assert_eq!(decoder.session().input_budget().unwrap().used(), 4);

    let mut many = String::from("{");
    for index in 0..4096 {
        if index != 0 {
            many.push(',');
        }
        many.push_str(&format!("\"key-{index}\":{index}"));
    }
    many.push('}');
    let value = JsonDecoder::unlimited()
        .decode_value_str(&many)
        .expect("4096 distinct keys must be preserved");
    assert_eq!(value.as_value().as_object().unwrap().len(), 4096);
    many.pop();
    many.push_str(",\"key-0\":0}");
    let error = JsonDecoder::unlimited().decode_value_str(&many).unwrap_err();
    assert_eq!(error.kind(), JsonDecodeErrorKind::Deserialize);

    let limits = JsonDecodeLimits::builder().max_nodes(2_usize).build();
    let mut decoder = JsonDecoder::with_limits(limits);
    let error = decoder.decode_value_str(r#"{"a":1,"b":2}"#).unwrap_err();
    assert_eq!(error.kind(), JsonDecodeErrorKind::Budget);
    assert_eq!(decoder.session().value_budget().used_nodes(), Some(0));
}

#[test]
fn test_validated_decode_keeps_nested_path_and_token_metadata() {
    let options = JsonValidatedDecodeOptions::builder().build();
    let mut seen = None;
    JsonDecoder::unlimited()
        .decode_unique_validated_str(r#"{"items":[{"n":7}]}"#, &options, |path, token| {
            seen = Some((path.to_vec(), token.raw().to_owned(), token.offset()));
            Ok::<_, std::convert::Infallible>(JsonNumberPolicy::Preserve)
        })
        .expect("validated preserve policy must decode");
    assert_eq!(
        seen,
        Some((
            vec![
                JsonPathSegment::Key("items".to_owned()),
                JsonPathSegment::Index(0),
                JsonPathSegment::Key("n".to_owned()),
            ],
            "7".to_owned(),
            15,
        ))
    );
}

#[derive(Debug, serde::Deserialize, PartialEq)]
#[serde(untagged)]
enum UniqueUntaggedValue {
    Empty(Vec<()>),
    Json(DuplicateKeyRejectingJsonValue),
}

#[derive(Debug, serde::Deserialize, PartialEq)]
enum UniqueEnum {
    Unit,
    New(f64),
    Tuple(f64, bool),
    Struct { amount: f64 },
}

#[derive(serde::Deserialize)]
struct UniqueNewtype(f64);

#[test]
fn test_unique_decoder_exposes_native_events_before_untagged_buffering() {
    let decoded: UniqueUntaggedValue = JsonDecoder::unlimited()
        .decode_unique_str(r#"{"amount":1.5,"$serde_json::private::RawValue":"null"}"#)
        .expect("untagged buffering must receive native numeric events");
    let UniqueUntaggedValue::Json(value) = decoded else {
        panic!("an object must select the JSON variant");
    };
    assert_eq!(
        value.into_inner(),
        json!({"amount": 1.5, "$serde_json::private::RawValue": "null"})
    );
}

#[test]
fn test_unique_decoder_supports_enum_newtype_and_raw_targets() {
    for (input, expected) in [
        (r#""Unit""#, UniqueEnum::Unit),
        (r#"{"New":1.5}"#, UniqueEnum::New(1.5)),
        (r#"{"Tuple":[1.5,true]}"#, UniqueEnum::Tuple(1.5, true)),
        (r#"{"Struct":{"amount":1.5}}"#, UniqueEnum::Struct { amount: 1.5 }),
    ] {
        assert_eq!(
            JsonDecoder::unlimited().decode_unique_str::<UniqueEnum>(input).unwrap(),
            expected
        );
    }
    assert_eq!(
        JsonDecoder::unlimited()
            .decode_unique_str::<UniqueNewtype>("1.5")
            .unwrap()
            .0,
        1.5
    );
    let raw: Box<serde_json::value::RawValue> = JsonDecoder::unlimited()
        .decode_unique_str(r#"{"amount":1.5}"#)
        .expect("owned RawValue targets must be reconstructed");
    assert_eq!(raw.get(), r#"{"amount":1.5}"#);
    for input in [r#"{"New":1.5,"Unit":null}"#, r#"{"Tuple":[1.5,true,7]}"#] {
        assert!(JsonDecoder::unlimited().decode_unique_str::<UniqueEnum>(input).is_err());
    }
}

#[test]
fn test_unique_decoder_retains_input_charges_and_rolls_back_value_failures() {
    let limits = JsonDecodeLimits::<JsonResource, usize>::builder()
        .max_input_bytes(64)
        .max_nodes(3)
        .build();
    let mut decoder = JsonDecoder::with_limits(limits);
    assert!(decoder.decode_unique_seed_utf8(FailingSeed, b"[1]").is_err());
    assert_eq!(decoder.session().value_budget().used_nodes(), Some(0));
    assert_eq!(decoder.session().input_budget().unwrap().used(), 3);
    let duplicate = "{\n \"x\":1,\"x\":2}";
    let error = decoder
        .decode_unique_str::<DuplicateKeyRejectingJsonValue>(duplicate)
        .unwrap_err();
    assert_eq!(error.kind(), JsonDecodeErrorKind::Deserialize);
    assert_eq!(error.line(), Some(2));
    assert_eq!(error.column(), Some(8));
    assert_eq!(decoder.session().value_budget().used_nodes(), Some(0));
    assert_eq!(decoder.session().input_budget().unwrap().used(), 3 + duplicate.len());
    assert_eq!(decoder.decode_unique_str::<Vec<u8>>("[1]").unwrap(), vec![1]);
    assert_eq!(decoder.session().value_budget().used_nodes(), Some(2));
    let error = decoder.decode_unique_str::<Vec<u8>>("[2]").unwrap_err();
    assert_eq!(error.kind(), JsonDecodeErrorKind::Budget);
    assert_eq!(decoder.session().value_budget().used_nodes(), Some(2));
}

#[test]
fn test_unique_decoder_uses_configured_depth_budget_and_preserves_syntax_admission() {
    let limits = JsonDecodeLimits::builder().max_depth(1_usize).build();
    let error = JsonDecoder::with_limits(limits)
        .decode_unique_str::<DuplicateKeyRejectingJsonValue>(&nested_array(1))
        .expect_err("configured root-inclusive depth budget must be enforced");
    assert_eq!(error.kind(), JsonDecodeErrorKind::Budget);
    assert!(
        JsonDecoder::unlimited()
            .decode_unique_str::<bool>("true false")
            .is_err()
    );
    let error = JsonDecoder::unlimited()
        .decode_unique_utf8::<bool>(&[0xff])
        .unwrap_err();
    assert_eq!(error.kind(), JsonDecodeErrorKind::InvalidUtf8);
}

#[test]
fn test_unique_decoder_materialization_uses_byte_coordinates() {
    for policy in [DiagnosticPolicy::Redacted, DiagnosticPolicy::Detailed] {
        for (input, line, column) in [
            ("{\"é\":0,\"é\":1}", 1, 9),
            ("{\n\"é\":0,\"é\":1}", 2, 8),
            ("{\r\n\"é\":0,\"é\":1}", 2, 8),
            ("{\r\"é\":0,\"é\":1}", 1, 10),
        ] {
            let error = JsonDecoder::unlimited()
                .with_diagnostic_policy(policy)
                .decode_unique_str::<DuplicateKeyRejectingJsonValue>(input)
                .unwrap_err();
            assert_eq!(error.kind(), JsonDecodeErrorKind::Deserialize);
            assert_eq!(error.stage(), JsonDecodeStage::Deserialize);
            assert_eq!(error.line(), Some(line), "input={input:?}");
            assert_eq!(error.column(), Some(column), "input={input:?}");
            if policy == DiagnosticPolicy::Detailed {
                let JsonDecodeErrorSource::Deserialize {
                    source: Some(source), ..
                } = error.into_source()
                else {
                    panic!("detailed diagnostics must retain the Serde source");
                };
                let source = source
                    .downcast_ref::<SerdeJsonError>()
                    .expect("detailed source must be a serde_json error");
                assert_eq!(source.line(), line, "input={input:?}");
                assert_eq!(source.column(), column, "input={input:?}");
            } else {
                assert!(error.source().is_none());
            }
        }
    }
}

#[test]
fn test_unique_decoder_accepts_legacy_depth_boundary() {
    let input = format!("{{\"é\":{}}}", nested_array(127));
    for policy in [DiagnosticPolicy::Redacted, DiagnosticPolicy::Detailed] {
        let value = JsonDecoder::unlimited()
            .with_diagnostic_policy(policy)
            .decode_unique_str::<DuplicateKeyRejectingJsonValue>(&input)
            .expect("legacy typed decode must not impose the old 127-level cap");
        assert!(value.into_inner().is_object());
    }
}

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
fn test_json_text_decoder_obeys_private_number_marker_semantics() {
    const MARKER: &str = concat!("$", "serde_json", "::private::Number");
    let input = format!(r#"{{"{MARKER}":"123"}}"#);
    let arbitrary_precision = serde_json::from_str::<Value>("1e400").is_ok();
    let limits = JsonDecodeLimits::<JsonResource, usize>::builder()
        .value_limits(
            JsonValueLimits::builder()
                .number_bytes_limit(ResourceLimit::new(
                    JsonResource::NumberBytes,
                    if arbitrary_precision { 3 } else { 1 },
                ))
                .build(),
        )
        .build();
    let value = JsonDecoder::with_limits(limits)
        .decode_str::<Value>(&input)
        .expect("Serde's private Number marker semantics must be honored");

    if arbitrary_precision {
        assert_eq!(value, json!(123));
    } else {
        assert_eq!(value, json!({MARKER: "123"}));
    }
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
    offset: usize,
    interrupt_once: bool,
    fail_after_chunks: bool,
    reads: usize,
}

impl OnlyInput {
    fn new(chunks: Vec<&'static [u8]>) -> Self {
        Self {
            chunks,
            next: 0,
            offset: 0,
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
        if count == 0 {
            return Ok(0);
        }
        while self.next < self.chunks.len() && self.offset == self.chunks[self.next].len() {
            self.next += 1;
            self.offset = 0;
        }
        if self.next == self.chunks.len() {
            return if self.fail_after_chunks {
                Err(IoError::other("sensitive source text"))
            } else {
                Ok(0)
            };
        }
        let chunk = self.chunks[self.next];
        let remaining = &chunk[self.offset..];
        let copied = remaining.len().min(count);
        output[index..index + copied].copy_from_slice(&remaining[..copied]);
        self.offset += copied;
        if self.offset == chunk.len() {
            self.next += 1;
            self.offset = 0;
        }
        Ok(copied)
    }
}

#[test]
fn test_only_input_preserves_chunk_remainder_across_small_reads() {
    let bytes = b"true false";
    let mut input = OnlyInput::new(vec![b"", b"true false", b""]);
    input.interrupt_once = true;
    let mut output = [0; 6];
    let mut read = Vec::new();

    loop {
        let count = match unsafe { input.read_unchecked(&mut output, 1, 4) } {
            Ok(count) => count,
            Err(error) if error.kind() == ErrorKind::Interrupted => continue,
            Err(error) => panic!("unexpected read error: {error}"),
        };
        if count == 0 {
            break;
        }
        read.extend_from_slice(&output[1..1 + count]);
    }

    assert_eq!(read, bytes);
    assert_eq!(input.reads, 5, "the interrupt, three data reads, and EOF are counted");
}

#[test]
fn test_only_input_zero_count_does_not_consume_input() {
    let mut input = OnlyInput::new(vec![b"true"]);
    let mut output = [0; 5];

    assert_eq!(unsafe { input.read_unchecked(&mut output, 1, 0) }.unwrap(), 0);
    assert_eq!(input.next, 0);
    assert_eq!(unsafe { input.read_unchecked(&mut output, 1, 4) }.unwrap(), 4);
    assert_eq!(&output[1..5], b"true");
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
fn test_json_decoder_stream_rejects_trailing_document_in_small_reads() {
    let mut input = OnlyInput::new(vec![b"true false"]);
    let error = JsonDecoder::unlimited()
        .decode_input::<Value, _>(&mut input)
        .expect_err("a trailing document in the same chunk must not be discarded");
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
    assert_eq!(second.next, 0);
    assert_eq!(second.offset, 2);
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

#[cfg(feature = "jcs")]
#[test]
fn test_jcs_decoder_reuses_stream_input_fixtures() {
    use qubit_json::decode::JcsDecodeError;
    use qubit_json::decode::JcsDecoder;

    let limits = JsonDecodeLimits::<JsonResource, usize>::builder()
        .max_input_bytes(4)
        .build();
    let mut decoder = JcsDecoder::with_limits(limits);
    let mut exact = OnlyInput::new(vec![b"true"]);
    exact.interrupt_once = true;
    assert_eq!(decoder.decode_input(&mut exact).unwrap(), Value::Bool(true));
    assert_eq!(exact.reads, 3);

    let limits = JsonDecodeLimits::<JsonResource, usize>::builder()
        .max_input_bytes(4)
        .build();
    let mut oversized_decoder = JcsDecoder::with_limits(limits);
    let mut oversized = OnlyInput::new(vec![b"false"]);
    let error = oversized_decoder.decode_input(&mut oversized).unwrap_err();
    let JcsDecodeError::Decode(error) = error else {
        panic!("expected input budget error")
    };
    assert_eq!(error.kind(), JsonDecodeErrorKind::Budget);
    assert_eq!(error.raw_input_bytes(), 4);
    assert_eq!(oversized_decoder.session().input_budget().unwrap().used(), 4);

    let mut failing = OnlyInput::new(vec![b"true"]);
    failing.fail_after_chunks = true;
    let error = JcsDecoder::unlimited().decode_input(&mut failing).unwrap_err();
    let JcsDecodeError::Decode(error) = error else {
        panic!("expected input read error")
    };
    assert_eq!(error.kind(), JsonDecodeErrorKind::Read);
    assert_eq!(error.raw_input_bytes(), 4);

    let mut interrupted = OnlyInput::new(vec![b"true"]);
    interrupted.interrupt_once = true;
    assert_eq!(
        JcsDecoder::unlimited().decode_input(&mut interrupted).unwrap(),
        Value::Bool(true)
    );
    assert_eq!(interrupted.reads, 3);
}

#[cfg(feature = "jcs")]
#[test]
fn test_jcs_decoder_depth_admission_precedes_backend_materialization() {
    use qubit_budget::StructureLimits;
    use qubit_json::decode::JcsDecodeError;
    use qubit_json::decode::JcsDecoder;

    let deep = nested_array(20_000);
    let limits = JsonDecodeLimits::<JsonResource, usize>::builder()
        .value_limits(
            JsonValueLimits::builder()
                .structure_limits(StructureLimits::builder().depth_limit(ResourceLimit::new(JsonResource::Depth, 128)))
                .build(),
        )
        .build();
    let error = JcsDecoder::with_limits(limits).decode_str(&deep).unwrap_err();
    let JcsDecodeError::Decode(error) = error else {
        panic!("expected lexical depth budget rejection")
    };
    assert_eq!(error.kind(), JsonDecodeErrorKind::Budget);
}

mod migrated_ordinary {
    use qubit_budget::json::JsonDecodeLimits;
    use qubit_budget::json::JsonResource;
    use qubit_json::decode::JsonDecodeNumberModel;
    use qubit_json::decode::JsonDecoder;
    use qubit_json::decode::JsonNumberPolicy;
    use qubit_json::decode::JsonValidatedDecodeError;
    use qubit_json::decode::JsonValidatedDecodeOptions;
    use serde::Deserialize;
    #[derive(Debug, Deserialize, PartialEq)]
    struct DecodeRecord {
        name: String,
        flags: Vec<bool>,
    }
    #[derive(Debug, Deserialize, PartialEq)]
    struct DecodeNewtype(u64);
    #[derive(Debug, Deserialize, PartialEq)]
    enum DecodeEnum {
        Unit,
        Tuple(u64, bool),
        Struct { value: String },
    }
    #[test]
    fn test_depth_budget_is_root_inclusive_and_has_no_implicit_cap() {
        for (limit, text, expected) in [
            (0_usize, "0", false),
            (1, "0", true),
            (1, "{}", true),
            (1, "[0]", false),
            (2, "[0]", true),
            (2, "[[0]]", false),
            (3, "[[0]]", true),
            (3, "{}", true),
        ] {
            let limits = JsonDecodeLimits::builder().max_depth(limit).build();
            let result = JsonDecoder::with_limits(limits).decode_value_str(text);
            assert_eq!(result.is_ok(), expected, "limit={limit}, text={text}");
        }
        for depth in [127_usize, 128, 129, 32_768] {
            let text = format!("{}0{}", "[".repeat(depth), "]".repeat(depth));
            let value = JsonDecoder::unlimited().decode_value_str(&text).unwrap();
            assert!(value.as_value().is_array());
            drop(value);
        }
    }

    #[test]
    fn test_deep_scanner_failures_drop_guarded_partial_values() {
        let truncated = format!("{}0", "[".repeat(512));
        assert!(JsonDecoder::unlimited().decode_value_str(&truncated).is_err());
        let deeply_truncated = format!("{}0", "[".repeat(32_768));
        assert!(JsonDecoder::unlimited().decode_value_str(&deeply_truncated).is_err());
        let duplicate = format!("{}{{\"x\":0,\"x\":1}}{}", "[".repeat(128), "]".repeat(128));
        assert!(JsonDecoder::unlimited().decode_value_str(&duplicate).is_err());
    }

    #[test]
    fn test_duplicate_key_precedes_colon_and_trailing_key_context_is_object_key() {
        let options = JsonValidatedDecodeOptions::builder().build();
        let duplicate = JsonDecoder::unlimited()
            .decode_unique_validated_str(r#"{"a":0,"a" 1}"#, &options, |_, _| {
                Ok::<_, ()>(JsonNumberPolicy::Preserve)
            })
            .unwrap_err();
        assert!(matches!(
            duplicate,
            JsonValidatedDecodeError::Decode {
                context: Some(qubit_json::decode::JsonDecodeContext::ObjectKey),
                ..
            }
        ));
        let trailing = JsonDecoder::unlimited()
            .decode_unique_validated_str(r#"{"a":true,}"#, &options, |_, _| {
                Ok::<_, ()>(JsonNumberPolicy::Preserve)
            })
            .unwrap_err();
        assert!(matches!(
            trailing,
            JsonValidatedDecodeError::Decode {
                context: Some(qubit_json::decode::JsonDecodeContext::ObjectKey),
                ..
            }
        ));
        let no_key_budget = JsonValidatedDecodeOptions::builder().max_value_and_key_nodes(1).build();
        let budget_first = JsonDecoder::unlimited()
            .decode_unique_validated_str(r#"{"a":true,}"#, &no_key_budget, |_, _| {
                Ok::<_, ()>(JsonNumberPolicy::Preserve)
            })
            .unwrap_err();
        assert!(matches!(
            budget_first,
            JsonValidatedDecodeError::Limit {
                limit: qubit_json::decode::JsonValidatedLimit::ValueAndKeyNodes,
                maximum: 1,
                observed: 2,
                ..
            }
        ));
    }

    #[test]
    fn test_dyn_and_chunked_byte_input_decode_once() {
        struct OneByteReader {
            bytes: &'static [u8],
            offset: usize,
        }
        impl std::io::Read for OneByteReader {
            fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
                if self.offset == self.bytes.len() {
                    return Ok(0);
                }
                buffer[0] = self.bytes[self.offset];
                self.offset += 1;
                Ok(1)
            }
        }
        let options = JsonValidatedDecodeOptions::builder().build();
        let mut reader = OneByteReader {
            bytes: b"[true,1]",
            offset: 0,
        };
        let input: &mut dyn qubit_io::ByteInput = &mut reader;
        let owner = JsonDecoder::unlimited()
            .decode_unique_validated_input(input, &options, |_, _| Ok::<_, ()>(JsonNumberPolicy::Preserve))
            .unwrap();
        assert!(owner.as_value().is_array());
        assert_eq!(reader.offset, 8);
    }

    #[test]
    fn test_input_utf8_io_and_one_byte_limit_probe() {
        let limits = JsonDecodeLimits::builder().max_input_bytes(16_usize).build();
        let mut decoder = JsonDecoder::with_limits(limits);
        let mut invalid = std::io::Cursor::new(vec![0xff]);
        let error = decoder.decode_value_input(&mut invalid).unwrap_err();
        assert_eq!(error.kind(), qubit_json::decode::JsonDecodeErrorKind::InvalidUtf8);
        assert_eq!(decoder.session().input_budget().unwrap().used(), 1);

        let limits = JsonDecodeLimits::builder().max_input_bytes(2_usize).build();
        let mut decoder = JsonDecoder::with_limits(limits);
        let mut oversized = std::io::Cursor::new(b"true".to_vec());
        let error = decoder.decode_value_input(&mut oversized).unwrap_err();
        assert_eq!(error.kind(), qubit_json::decode::JsonDecodeErrorKind::Budget);
        assert_eq!(decoder.session().input_budget().unwrap().used(), 2);
        assert_eq!(oversized.position(), 3);

        struct FailsAfterTwo {
            offset: usize,
        }
        impl std::io::Read for FailsAfterTwo {
            fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
                if self.offset >= 2 {
                    return Err(std::io::Error::other("input failure"));
                }
                output[0] = b'[';
                self.offset += 1;
                Ok(1)
            }
        }
        let limits = JsonDecodeLimits::builder().max_input_bytes(16_usize).build();
        let mut decoder = JsonDecoder::with_limits(limits);
        let error = decoder
            .decode_value_input(&mut FailsAfterTwo { offset: 0 })
            .unwrap_err();
        assert_eq!(error.kind(), qubit_json::decode::JsonDecodeErrorKind::Read);
        assert_eq!(decoder.session().input_budget().unwrap().used(), 2);
    }

    #[test]
    fn test_old_typed_and_unique_decode_keep_distinct_duplicate_semantics() {
        let mut decoder = JsonDecoder::unlimited();
        let value: serde_json::Value = decoder.decode_str(r#"{"x":1,"x":2}"#).unwrap();
        assert_eq!(value["x"], 2);
        let error = decoder
            .decode_unique_str::<serde_json::Value>(r#"{"x":1,"x":2}"#)
            .unwrap_err();
        assert_eq!(
            error.materialization_error_reason(),
            Some(qubit_json::decode::JsonMaterializationErrorReason::DuplicateKey)
        );

        let raw: Box<serde_json::value::RawValue> = decoder.decode_unique_str(r#"{"x":1}"#).unwrap();
        assert_eq!(raw.get(), r#"{"x":1}"#);
    }

    #[test]
    fn test_old_unique_targets_and_error_precedence_are_preserved() {
        let mut decoder = JsonDecoder::unlimited();
        let unit: () = decoder.decode_unique_str("null").unwrap();
        assert_eq!(unit, ());
        let newtype: DecodeNewtype = decoder.decode_unique_str("7").unwrap();
        assert_eq!(newtype, DecodeNewtype(7));
        let tuple: (u64, bool) = decoder.decode_unique_str("[7,true]").unwrap();
        assert_eq!(tuple, (7, true));
        let record: DecodeRecord = decoder
            .decode_unique_str(r#"{"name":"item","flags":[true,false]}"#)
            .unwrap();
        assert_eq!(
            record,
            DecodeRecord {
                name: "item".to_owned(),
                flags: vec![true, false]
            }
        );
        let unit_variant: DecodeEnum = decoder.decode_unique_str(r#""Unit""#).unwrap();
        let tuple_variant: DecodeEnum = decoder.decode_unique_str(r#"{"Tuple":[7,true]}"#).unwrap();
        let struct_variant: DecodeEnum = decoder.decode_unique_str(r#"{"Struct":{"value":"x"}}"#).unwrap();
        assert_eq!(unit_variant, DecodeEnum::Unit);
        assert_eq!(tuple_variant, DecodeEnum::Tuple(7, true));
        assert_eq!(struct_variant, DecodeEnum::Struct { value: "x".to_owned() });

        let type_error = decoder.decode_unique_str::<u64>(r#""wrong""#).unwrap_err();
        assert_eq!(type_error.kind(), qubit_json::decode::JsonDecodeErrorKind::Deserialize);

        let syntax_first = decoder
            .decode_unique_str::<serde_json::Value>(r#"{"x":0,"x":1} trailing"#)
            .unwrap_err();
        assert_eq!(
            syntax_first.kind(),
            qubit_json::decode::JsonDecodeErrorKind::InvalidJson
        );
        assert_eq!(syntax_first.materialization_error_reason(), None);

        let duplicate_with_location = JsonDecoder::unlimited()
            .with_diagnostic_policy(qubit_json::decode::DiagnosticPolicy::Detailed)
            .decode_unique_str::<serde_json::Value>("{\n\"x\":0,\"x\":1}")
            .unwrap_err();
        assert_eq!(
            duplicate_with_location.materialization_error_reason(),
            Some(qubit_json::decode::JsonMaterializationErrorReason::DuplicateKey),
            "{duplicate_with_location}"
        );
        assert!(duplicate_with_location.to_string().contains("line 2 column 7"));
    }

    #[test]
    fn test_decode_error_source_obeys_diagnostic_policy() {
        struct ReadFailure;
        impl std::io::Read for ReadFailure {
            fn read(&mut self, _output: &mut [u8]) -> std::io::Result<usize> {
                Err(std::io::Error::other("secret read detail"))
            }
        }

        let mut input = ReadFailure;
        let error = JsonDecoder::unlimited()
            .with_diagnostic_policy(qubit_json::decode::DiagnosticPolicy::Detailed)
            .decode_value_input(&mut input)
            .unwrap_err();
        assert_eq!(error.kind(), qubit_json::decode::JsonDecodeErrorKind::Read);
        assert!(std::error::Error::source(&error).is_some());
        assert!(!error.to_string().contains("secret read detail"));

        let mut input = ReadFailure;
        let error = JsonDecoder::unlimited().decode_value_input(&mut input).unwrap_err();
        assert_eq!(error.kind(), qubit_json::decode::JsonDecodeErrorKind::Read);
        assert!(std::error::Error::source(&error).is_none());
        assert!(!error.to_string().contains("secret read detail"));
    }

    #[test]
    fn test_decoder_uses_custom_quantity_for_typed_input_and_validated_paths() {
        let mut decoder = JsonDecoder::with_limits(JsonDecodeLimits::<JsonResource, u8>::new());

        let mut typed_input = std::io::Cursor::new(br#"{"n":7}"#.to_vec());
        let typed: serde_json::Value = decoder
            .decode_input_with_number_model(&mut typed_input, JsonDecodeNumberModel::Integer64)
            .unwrap();
        assert_eq!(typed, serde_json::json!({"n": 7}));

        let guarded = decoder.decode_value_utf8(br#"[true,null]"#).unwrap();
        assert_eq!(guarded.as_value(), &serde_json::json!([true, null]));

        let options = JsonValidatedDecodeOptions::builder().build();
        let validated = decoder
            .decode_unique_validated_utf8(b"[1,2]", &options, |_, _| {
                Ok::<_, std::convert::Infallible>(JsonNumberPolicy::Preserve)
            })
            .unwrap();
        assert_eq!(validated.as_value(), &serde_json::json!([1, 2]));
    }

    #[test]
    fn test_legacy_typed_decoder_facades_share_the_document_contract() {
        let mut decoder = JsonDecoder::unlimited();

        let text: serde_json::Value = decoder.decode_str(r#"{"n":1}"#).unwrap();
        assert_eq!(text, serde_json::json!({"n": 1}));

        let utf8: serde_json::Value = decoder.decode_utf8(br#"{"n":2}"#).unwrap();
        assert_eq!(utf8, serde_json::json!({"n": 2}));

        let mut input = std::io::Cursor::new(br#"{"n":3}"#.to_vec());
        let streamed: serde_json::Value = decoder.decode_input(&mut input).unwrap();
        assert_eq!(streamed, serde_json::json!({"n": 3}));

        let object: serde_json::Map<String, serde_json::Value> = decoder.decode_object_str(r#"{"n":4}"#).unwrap();
        assert_eq!(object.get("n"), Some(&serde_json::json!(4)));

        let object: serde_json::Map<String, serde_json::Value> = decoder.decode_object_utf8(br#"{"n":5}"#).unwrap();
        assert_eq!(object.get("n"), Some(&serde_json::json!(5)));

        let array: Vec<serde_json::Value> = decoder.decode_array_str("[6]").unwrap();
        assert_eq!(array, [serde_json::json!(6)]);

        let array: Vec<serde_json::Value> = decoder.decode_array_utf8(b"[7]").unwrap();
        assert_eq!(array, [serde_json::json!(7)]);

        decoder.validate_str(r#"{"ok":true}"#).unwrap();
        decoder.validate_utf8(br#"{"ok":false}"#).unwrap();
    }
}
