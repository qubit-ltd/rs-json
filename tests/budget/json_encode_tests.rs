// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests for budget-aware JSON encoding.

use std::cell::Cell;
use std::io;
use std::io::Write;

use qubit_budget::ResourceLimit;
use qubit_budget::StructureLimits;
use qubit_budget::json::JsonEncodeLimits;
use qubit_budget::json::JsonEncodeSession;
use qubit_budget::json::JsonResource;
use qubit_budget::json::JsonValueLimits;
use qubit_json::text::JsonEncodeError;
use qubit_json::text::encode_to_vec;
use qubit_json::text::encode_to_writer;
use qubit_json::text::encode_to_writer_incremental;
use serde::Serialize;
use serde::Serializer;
use serde::ser::Error as _;
use serde::ser::SerializeMap;
use serde::ser::SerializeSeq;
use serde_json::Number;
use serde_json::json;
use serde_json::value::RawValue;

use super::json_test_limits_tests::JsonTestLimits;

/// Arbitrary-precision number text used by online accounting tests.
const LARGE_NUMBER_TEXT: &str = "123456789012345678901234567890";

/// Value that emits a prefix before returning a custom Serde error.
struct FailsAfterPrefix;

impl Serialize for FailsAfterPrefix {
    /// Emits one sequence item, then fails before completing the sequence.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut sequence = serializer.serialize_seq(None)?;
        sequence.serialize_element(&1_u8)?;
        Err(S::Error::custom("deliberate serialization failure"))
    }
}

/// Unknown-length sequence with observable element traversal.
struct CountedSequence<'a> {
    /// Number of elements whose serialization was entered.
    serialized: &'a Cell<usize>,

    /// Number of elements offered by the source.
    len: usize,
}

impl Serialize for CountedSequence<'_> {
    /// Emits an unknown-length sequence while recording each entered element.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut sequence = serializer.serialize_seq(None)?;
        for value in 0..self.len {
            self.serialized.set(self.serialized.get() + 1);
            sequence.serialize_element(&value)?;
        }
        sequence.end()
    }
}

/// Recursively emitted sequence with observable traversal depth.
struct CountedDepth<'a> {
    /// Number of values whose serialization was entered.
    serialized: &'a Cell<usize>,

    /// Nested sequence levels remaining below this value.
    remaining: usize,
}

impl Serialize for CountedDepth<'_> {
    /// Emits one nested child until the requested source depth is reached.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.serialized.set(self.serialized.get() + 1);
        if self.remaining == 0 {
            serializer.serialize_unit()
        } else {
            let mut sequence = serializer.serialize_seq(Some(1))?;
            sequence.serialize_element(&CountedDepth {
                serialized: self.serialized,
                remaining: self.remaining - 1,
            })?;
            sequence.end()
        }
    }
}

/// Sequence containing one checked value before an observable tail.
struct SequenceThenTail<'a, T: ?Sized> {
    /// Value expected to fail a budget check.
    first: &'a T,

    /// Number of times serialization reached the tail.
    serialized_tail: &'a Cell<usize>,
}

impl<T> Serialize for SequenceThenTail<'_, T>
where
    T: Serialize + ?Sized,
{
    /// Emits the checked value before recording entry into the tail.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut sequence = serializer.serialize_seq(Some(2))?;
        sequence.serialize_element(self.first)?;
        self.serialized_tail.set(self.serialized_tail.get() + 1);
        sequence.serialize_element(&())?;
        sequence.end()
    }
}

/// Map containing one checked entry before an observable tail.
struct MapThenTail<'a, K: ?Sized, V: ?Sized> {
    /// Key expected to pass or fail its budget check.
    key: &'a K,

    /// Value expected to pass or fail its budget check.
    value: &'a V,

    /// Number of times serialization reached the tail entry.
    serialized_tail: &'a Cell<usize>,
}

impl<K, V> Serialize for MapThenTail<'_, K, V>
where
    K: Serialize + ?Sized,
    V: Serialize + ?Sized,
{
    /// Emits the checked entry before recording entry into the tail.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(2))?;
        map.serialize_entry(self.key, self.value)?;
        self.serialized_tail.set(self.serialized_tail.get() + 1);
        map.serialize_entry("tail", &())?;
        map.end()
    }
}

/// Map that intentionally omits its entry-count hint.
struct UnknownMap(usize);

impl Serialize for UnknownMap {
    /// Emits integer key/value pairs through an unknown-length map.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(None)?;
        for value in 0..self.0 {
            map.serialize_entry(&value, &value)?;
        }
        map.end()
    }
}

/// Writer that accepts a bounded prefix, then fails every later write.
struct PrefixThenFailWriter {
    /// Bytes accepted before the configured failure boundary.
    bytes: Vec<u8>,

    /// Maximum number of bytes accepted across all writes.
    accepted: usize,
}

impl Write for PrefixThenFailWriter {
    /// Accepts at most the remaining prefix capacity, then reports an error.
    fn write(&mut self, input: &[u8]) -> io::Result<usize> {
        let remaining = self.accepted.saturating_sub(self.bytes.len());
        if remaining == 0 {
            return Err(io::Error::other("deliberate writer failure"));
        }
        let accepted = remaining.min(input.len());
        self.bytes.extend_from_slice(&input[..accepted]);
        Ok(accepted)
    }

    /// Flushes without additional side effects.
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Asserts a budget error occurs before traversal reaches a source tail.
fn assert_online_rejection<T>(
    value: &T,
    limits: JsonTestLimits,
    expected: JsonResource,
    serialized_tail: &Cell<usize>,
) where
    T: Serialize + ?Sized,
{
    let mut session = limits.encode_session();
    let error = encode_to_vec(value, &mut session)
        .expect_err("the first value must be rejected online");
    let JsonEncodeError::Budget(error) = error else {
        panic!("expected a budget error, got {error:?}");
    };
    assert_eq!(
        error
            .budget_error()
            .expect("the error must contain a budget failure")
            .resource(),
        &expected,
    );
    assert_eq!(serialized_tail.get(), 0);
}

/// Verifies a budget failure leaves the destination writer unchanged.
#[test]
fn test_encode_to_writer_failure_does_not_touch_external_writer() {
    let limits = JsonEncodeLimits::empty().with_output_bytes_limit(
        ResourceLimit::new(JsonResource::OutputBytes, 3),
    );
    let mut session = JsonEncodeSession::owned(limits);
    let mut output = Vec::new();

    let error = encode_to_writer(&mut output, &"long", &mut session)
        .expect_err("the encoded string must exceed the output budget");

    assert!(matches!(error, JsonEncodeError::Budget(_)));
    assert!(output.is_empty());
}

/// Verifies a RawValue is traversed once and emitted without metadata charges.
#[test]
fn test_encode_to_vec_counts_raw_value_once() {
    let raw = RawValue::from_string(String::from(r#"{"k":"v"}"#))
        .expect("the fixture must be valid raw JSON");
    let mut session = JsonEncodeSession::owned(JsonEncodeLimits::empty());

    let output = encode_to_vec(raw.as_ref(), &mut session)
        .expect("the raw JSON value must encode");

    assert_eq!(output, br#"{"k":"v"}"#);
}

/// Verifies a custom Serde failure does not commit its buffered prefix.
#[test]
fn test_encode_to_writer_serde_failure_does_not_touch_external_writer() {
    let mut session = JsonEncodeSession::owned(JsonEncodeLimits::empty());
    let mut output = Vec::new();

    let error = encode_to_writer(&mut output, &FailsAfterPrefix, &mut session)
        .expect_err("the custom serializer must fail");

    assert!(matches!(error, JsonEncodeError::Serialize(_)));
    assert!(output.is_empty());
}

/// Verifies a failed encode does not consume borrowed output budget capacity.
#[test]
fn test_encode_to_vec_serde_failure_does_not_consume_output_budget() {
    let value_limits = JsonValueLimits::empty().with_structure_limits(
        StructureLimits::empty()
            .with_nodes_limit(ResourceLimit::new(JsonResource::Nodes, 16)),
    );
    let limits = JsonEncodeLimits::empty()
        .with_output_bytes_limit(ResourceLimit::new(
            JsonResource::OutputBytes,
            16,
        ))
        .with_value_limits(value_limits);
    let mut session = JsonEncodeSession::owned(limits);
    encode_to_vec(&true, &mut session).expect("the initial value should fit");
    let used_before = session
        .output_budget()
        .expect("output accounting should be configured")
        .used();
    let nodes_before = session.value_budget().structure_budget().used_nodes();

    let error = encode_to_vec(&FailsAfterPrefix, &mut session)
        .expect_err("the custom serializer must fail");

    assert!(matches!(error, JsonEncodeError::Serialize(_)));
    let output = session
        .output_budget()
        .expect("output accounting should remain configured");
    assert_eq!(output.used(), used_before);
    assert_eq!(output.remaining(), 16 - used_before);
    assert!(
        session.value_budget().structure_budget().used_nodes() > nodes_before,
        "accepted value work remains charged after serialization failure",
    );
}

/// Verifies a known map length is checked before its entries are traversed.
#[test]
fn test_encode_to_vec_known_map_limit_stops_before_source_tail() {
    let serialized_tail = Cell::new(0);
    let value = MapThenTail {
        key: &"first",
        value: &1_u8,
        serialized_tail: &serialized_tail,
    };

    assert_online_rejection(
        &value,
        JsonTestLimits::new().with_max_map_entries(1),
        JsonResource::MapEntries,
        &serialized_tail,
    );
}

/// Verifies output rejection stops traversal before a long source tail.
#[test]
fn test_encode_to_vec_output_limit_stops_before_source_tail() {
    let serialized = Cell::new(0);
    let value = CountedSequence {
        serialized: &serialized,
        len: 1_000,
    };
    let limits = JsonEncodeLimits::empty().with_output_bytes_limit(
        ResourceLimit::new(JsonResource::OutputBytes, 8),
    );
    let mut session = JsonEncodeSession::owned(limits);

    let error = encode_to_vec(&value, &mut session)
        .expect_err("the output budget must reject the long sequence");

    assert!(matches!(error, JsonEncodeError::Budget(_)));
    assert!(serialized.get() < value.len);
}

/// Verifies final writer I/O can leave an accepted prefix in the destination.
#[test]
fn test_encode_to_writer_io_failure_can_leave_partial_output() {
    let mut session = JsonEncodeSession::owned(JsonEncodeLimits::empty());
    let mut writer = PrefixThenFailWriter {
        bytes: Vec::new(),
        accepted: 2,
    };

    let error = encode_to_writer(&mut writer, &[1_u8, 2_u8], &mut session)
        .expect_err("the destination writer must fail during final commit");

    assert!(matches!(error, JsonEncodeError::Write(_)));
    assert_eq!(writer.bytes, b"[1");
}

/// Verifies incremental encoding matches transactional encoding on success.
#[test]
fn test_encode_to_writer_incremental_matches_encode_to_vec() {
    let value = json!({"items": [1, true, "text"]});
    let mut expected_session =
        JsonEncodeSession::owned(JsonEncodeLimits::empty());
    let expected = encode_to_vec(&value, &mut expected_session)
        .expect("transactional encoding should succeed");
    let mut session = JsonEncodeSession::owned(JsonEncodeLimits::empty());
    let mut output = Vec::new();

    encode_to_writer_incremental(&mut output, &value, &mut session)
        .expect("incremental encoding should succeed");

    assert_eq!(output, expected);
}

/// Verifies incremental encoding may leave accepted output on a budget error.
#[test]
fn test_encode_to_writer_incremental_preserves_partial_output_on_budget_error()
{
    let limits = JsonEncodeLimits::empty().with_output_bytes_limit(
        ResourceLimit::new(JsonResource::OutputBytes, 4),
    );
    let mut session = JsonEncodeSession::owned(limits);
    let mut output = Vec::new();

    let error = encode_to_writer_incremental(
        &mut output,
        &json!([1, 2, 3]),
        &mut session,
    )
    .expect_err("the output limit must reject the document");

    assert!(matches!(error, JsonEncodeError::Budget(_)));
    assert_eq!(output, b"[1,2");
    assert_eq!(
        session
            .output_budget()
            .expect("output budget should remain configured")
            .used(),
        4,
    );
}

/// Verifies incremental encoding maps destination failures to `Write`.
#[test]
fn test_encode_to_writer_incremental_preserves_partial_output_on_io_error() {
    let mut session = JsonEncodeSession::owned(JsonEncodeLimits::empty());
    let mut writer = PrefixThenFailWriter {
        bytes: Vec::new(),
        accepted: 2,
    };

    let error =
        encode_to_writer_incremental(&mut writer, &[1_u8, 2_u8], &mut session)
            .expect_err("the destination writer must fail incrementally");

    assert!(matches!(error, JsonEncodeError::Write(_)));
    assert_eq!(writer.bytes, b"[1");
}

/// Verifies incremental Serde failures retain their accepted prefix and usage.
#[test]
fn test_encode_to_writer_incremental_preserves_partial_output_on_serde_error() {
    let limits = JsonEncodeLimits::empty().with_output_bytes_limit(
        ResourceLimit::new(JsonResource::OutputBytes, 16),
    );
    let mut session = JsonEncodeSession::owned(limits);
    let mut output = Vec::new();

    let error = encode_to_writer_incremental(
        &mut output,
        &FailsAfterPrefix,
        &mut session,
    )
    .expect_err("the custom serializer must fail after its prefix");

    assert!(matches!(error, JsonEncodeError::Serialize(_)));
    assert_eq!(output, b"[1");
    assert_eq!(
        session
            .output_budget()
            .expect("output budget should remain configured")
            .used(),
        2,
    );
}

/// Verifies node accounting stops a long source before it is exhausted.
#[test]
fn test_encode_to_vec_node_limit_stops_before_source_tail() {
    let serialized = Cell::new(0);
    let value = CountedSequence {
        serialized: &serialized,
        len: 1_000,
    };
    let mut session = JsonTestLimits::new().with_max_nodes(3).encode_session();

    let error = encode_to_vec(&value, &mut session)
        .expect_err("the node budget must reject the long sequence");

    assert!(matches!(error, JsonEncodeError::Budget(_)));
    assert!(serialized.get() < value.len);
}

/// Verifies depth accounting stops recursive serialization online.
#[test]
fn test_encode_to_vec_depth_limit_stops_before_source_tail() {
    const SOURCE_DEPTH: usize = 128;

    let serialized = Cell::new(0);
    let value = CountedDepth {
        serialized: &serialized,
        remaining: SOURCE_DEPTH - 1,
    };
    let mut session = JsonTestLimits::new().with_max_depth(4).encode_session();

    let error = encode_to_vec(&value, &mut session)
        .expect_err("the depth budget must reject recursive serialization");

    assert!(matches!(error, JsonEncodeError::Budget(_)));
    assert!(serialized.get() < SOURCE_DEPTH);
}

/// Verifies arbitrary-precision number rejection occurs before a later value.
#[test]
fn test_encode_to_vec_number_limit_stops_before_source_tail() {
    let number = LARGE_NUMBER_TEXT
        .parse::<Number>()
        .expect("the number fixture must parse");
    let serialized_tail = Cell::new(0);
    let value = SequenceThenTail {
        first: &number,
        serialized_tail: &serialized_tail,
    };

    assert_online_rejection(
        &value,
        JsonTestLimits::new()
            .with_max_number_bytes(LARGE_NUMBER_TEXT.len() - 1),
        JsonResource::NumberBytes,
        &serialized_tail,
    );
}

/// Verifies UTF-8 key rejection occurs before a later map entry.
#[test]
fn test_encode_to_vec_key_limit_stops_before_source_tail() {
    let key = "a\n\"你";
    let serialized_tail = Cell::new(0);
    let value = MapThenTail {
        key: &key,
        value: &(),
        serialized_tail: &serialized_tail,
    };

    assert_online_rejection(
        &value,
        JsonTestLimits::new().with_max_key_bytes(key.len() - 1),
        JsonResource::KeyBytes,
        &serialized_tail,
    );
}

/// Verifies UTF-8 string rejection occurs before a later sequence value.
#[test]
fn test_encode_to_vec_string_limit_stops_before_source_tail() {
    let text = "x\t好";
    let serialized_tail = Cell::new(0);
    let value = SequenceThenTail {
        first: &text,
        serialized_tail: &serialized_tail,
    };

    assert_online_rejection(
        &value,
        JsonTestLimits::new().with_max_string_bytes(text.len() - 1),
        JsonResource::StringBytes,
        &serialized_tail,
    );
}

/// Verifies an unknown map rejects at its actual online boundary.
#[test]
fn test_encode_to_vec_unknown_map_limit_stops_before_source_tail() {
    let map = UnknownMap(2);
    let serialized_tail = Cell::new(0);
    let value = SequenceThenTail {
        first: &map,
        serialized_tail: &serialized_tail,
    };

    assert_online_rejection(
        &value,
        JsonTestLimits::new().with_max_map_entries(1),
        JsonResource::MapEntries,
        &serialized_tail,
    );
}
