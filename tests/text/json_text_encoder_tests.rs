// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests budget-aware JSON text encoding.

use std::cell::Cell;
use std::io;
use std::io::Write;
use std::panic;

use qubit_budget::ResourceBudget;
use qubit_budget::ResourceLimit;
use qubit_budget::StructureLimits;
use qubit_budget::json::JsonEncodeLimits;
use qubit_budget::json::JsonEncodeSession;
use qubit_budget::json::JsonResource;
use qubit_budget::json::JsonValueBudget;
use qubit_budget::json::JsonValueLimits;
use qubit_json::text::JsonEncodeError;
use qubit_json::text::JsonTextEncoder;
use serde::Serialize;
use serde::Serializer;
use serde::ser::Error as _;
use serde::ser::SerializeMap;
use serde::ser::SerializeSeq;
use serde_json::Number;
use serde_json::json;
use serde_json::value::RawValue;

use crate::fixtures::JsonTestLimits;
use crate::text::json_encode_test_support::encode;
use crate::text::json_encode_test_support::write_buffered;
use crate::text::json_encode_test_support::write_incremental;

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

/// Value that emits a prefix before panicking during serialization.
struct PanicsAfterPrefix;

impl Serialize for PanicsAfterPrefix {
    /// Emits one sequence item, then panics before completing the sequence.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut sequence = serializer.serialize_seq(None)?;
        sequence
            .serialize_element(&1_u8)
            .expect("the prefix must serialize before the injected panic");
        panic!("deliberate serialization panic");
    }
}

/// Unknown-length sequence with observable element traversal.
struct CountedSequence<'a> {
    /// Number of elements whose serialization was entered.
    serialized: &'a Cell<usize>,

    /// Number of elements offered by the source.
    len: usize,
}

/// Sequence element that records entry into its `Serialize` implementation.
struct CountedElement<'a> {
    /// Number of element serializers entered.
    serialized: &'a Cell<usize>,
}

impl Serialize for CountedElement<'_> {
    /// Records the call before emitting a JSON null.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.serialized.set(self.serialized.get() + 1);
        serializer.serialize_unit()
    }
}

/// Unknown-length sequence used to verify pre-delegation item checks.
struct CountedElementSequence<'a> {
    /// Number of element serializers entered.
    serialized: &'a Cell<usize>,
    /// Number of elements offered by the source.
    len: usize,
}

impl Serialize for CountedElementSequence<'_> {
    /// Emits all source elements through the wrapped serializer.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut sequence = serializer.serialize_seq(None)?;
        for _ in 0..self.len {
            sequence.serialize_element(&CountedElement {
                serialized: self.serialized,
            })?;
        }
        sequence.end()
    }
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
struct PrefixWriter {
    /// Bytes accepted before the configured failure boundary.
    accepted: Vec<u8>,

    /// Maximum number of bytes accepted across all writes.
    maximum: usize,
}

impl Write for PrefixWriter {
    /// Accepts at most the remaining prefix capacity, then reports an error.
    fn write(&mut self, input: &[u8]) -> io::Result<usize> {
        if self.accepted.len() == self.maximum {
            return Err(io::Error::other("injected writer failure"));
        }
        let count = input.len().min(self.maximum - self.accepted.len());
        self.accepted.extend_from_slice(&input[..count]);
        Ok(count)
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
    expected_tail: usize,
) where
    T: Serialize + ?Sized,
{
    let mut session = limits.encode_session();
    let error = encode(value, &mut session).expect_err("the first value must be rejected online");
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
    assert_eq!(serialized_tail.get(), expected_tail);
}

/// Verifies a budget failure leaves the destination writer unchanged.
#[test]
fn test_write_buffered_failure_does_not_touch_external_writer() {
    let limits = JsonEncodeLimits::empty()
        .with_output_bytes_limit(ResourceLimit::new(JsonResource::OutputBytes, 3))
        .with_value_limits(JsonValueLimits::empty().with_structure_limits(
            StructureLimits::new().with_nodes_limit(ResourceLimit::new(JsonResource::Nodes, 16)),
        ));
    let mut session = JsonEncodeSession::owned(limits);
    let mut output = Vec::new();

    let error = write_buffered(&mut output, &"long", &mut session)
        .expect_err("the encoded string must exceed the output budget");

    assert!(matches!(error, JsonEncodeError::Budget(_)));
    assert!(output.is_empty());
    assert_eq!(
        session
            .output_budget()
            .expect("output budget should remain configured")
            .used(),
        0,
    );
    assert_eq!(session.value_budget().used_nodes(), Some(0));
}

/// Verifies a RawValue is traversed once and emitted without metadata charges.
#[test]
fn test_encode_counts_raw_value_once() {
    let raw = RawValue::from_string(String::from(r#"{"k":"v"}"#))
        .expect("the fixture must be valid raw JSON");
    let mut session = JsonEncodeSession::owned(JsonEncodeLimits::empty());

    let output = JsonTextEncoder::new(&mut session)
        .to_vec(raw.as_ref())
        .expect("the raw JSON value must encode");

    assert_eq!(output, br#"{"k":"v"}"#);
}

/// Verifies a custom Serde failure does not commit its buffered prefix.
#[test]
fn test_write_buffered_serde_failure_does_not_touch_external_writer() {
    let mut session = JsonEncodeSession::owned(JsonEncodeLimits::empty());
    let mut output = Vec::new();

    let error = write_buffered(&mut output, &FailsAfterPrefix, &mut session)
        .expect_err("the custom serializer must fail");

    assert!(matches!(error, JsonEncodeError::Serialize(_)));
    assert!(output.is_empty());
}

/// Verifies a failed Vec encode rolls back borrowed output and value budgets.
#[test]
fn test_encode_serde_failure_rolls_back_borrowed_budgets() {
    let mut output = ResourceBudget::new(JsonResource::OutputBytes, 16);
    let mut value = JsonValueBudget::new(JsonValueLimits::empty().with_structure_limits(
        StructureLimits::new().with_nodes_limit(ResourceLimit::new(JsonResource::Nodes, 16)),
    ));
    let mut session = JsonEncodeSession::borrowing_output(&mut output, &mut value);

    let error =
        encode(&FailsAfterPrefix, &mut session).expect_err("the custom serializer must fail");

    assert!(matches!(error, JsonEncodeError::Serialize(_)));
    drop(session);
    assert_eq!(output.used(), 0);
    assert_eq!(value.used_nodes(), Some(0));
}

/// Verifies Vec output-budget rejection leaves all caller-owned budgets
/// unchanged.
#[test]
fn test_encode_output_budget_rejection_rolls_back_borrowed_budgets() {
    let mut output = ResourceBudget::new(JsonResource::OutputBytes, 3);
    let mut value = JsonValueBudget::new(JsonValueLimits::empty().with_structure_limits(
        StructureLimits::new().with_nodes_limit(ResourceLimit::new(JsonResource::Nodes, 16)),
    ));
    let mut session = JsonEncodeSession::borrowing_output(&mut output, &mut value);

    let error = encode(&[1_u8, 2_u8], &mut session)
        .expect_err("the complete output must exceed the configured limit");

    assert!(matches!(error, JsonEncodeError::Budget(_)));
    drop(session);
    assert_eq!(output.used(), 0);
    assert_eq!(value.used_nodes(), Some(0));
}

/// Verifies a known map length is checked before its entries are traversed.
#[test]
fn test_encode_known_map_limit_stops_before_source_tail() {
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
        1,
    );
}

/// Verifies an unknown sequence rejects the next item before entering its
/// serializer.
#[test]
fn test_encode_sequence_limit_stops_before_next_serialize() {
    let serialized = Cell::new(0);
    let value = CountedElementSequence {
        serialized: &serialized,
        len: 2,
    };

    assert_online_rejection(
        &value,
        JsonTestLimits::new().with_max_sequence_items(1),
        JsonResource::SequenceItems,
        &serialized,
        1,
    );
}

/// Verifies output rejection stops traversal before a long source tail.
#[test]
fn test_encode_output_limit_stops_before_source_tail() {
    let serialized = Cell::new(0);
    let value = CountedSequence {
        serialized: &serialized,
        len: 1_000,
    };
    let limits = JsonEncodeLimits::empty()
        .with_output_bytes_limit(ResourceLimit::new(JsonResource::OutputBytes, 8));
    let mut session = JsonEncodeSession::owned(limits);

    let error =
        encode(&value, &mut session).expect_err("the output budget must reject the long sequence");

    assert!(matches!(error, JsonEncodeError::Budget(_)));
    assert!(serialized.get() < value.len);
}

/// Verifies final writer I/O can leave an accepted prefix in the destination.
#[test]
fn test_write_buffered_io_failure_can_leave_partial_output() {
    let mut output = ResourceBudget::new(JsonResource::OutputBytes, 16);
    let mut value = JsonValueBudget::new(JsonValueLimits::empty().with_structure_limits(
        StructureLimits::new().with_nodes_limit(ResourceLimit::new(JsonResource::Nodes, 16)),
    ));
    let mut session = JsonEncodeSession::borrowing_output(&mut output, &mut value);
    let mut writer = PrefixWriter {
        accepted: Vec::new(),
        maximum: 2,
    };

    let error = write_buffered(&mut writer, &[1_u8, 2_u8], &mut session)
        .expect_err("the destination writer must fail during final commit");

    assert!(matches!(error, JsonEncodeError::Write(_)));
    assert_eq!(writer.accepted, b"[1");
    drop(session);
    assert_eq!(output.used(), writer.accepted.len());
    assert_eq!(value.used_nodes(), Some(0));
}

/// Verifies incremental encoding matches transactional encoding on success.
#[test]
fn test_write_incremental_matches_encode() {
    let value = json!({"items": [1, true, "text"]});
    let mut expected_session = JsonEncodeSession::owned(JsonEncodeLimits::empty());
    let expected =
        encode(&value, &mut expected_session).expect("transactional encoding should succeed");
    let mut session = JsonEncodeSession::owned(JsonEncodeLimits::empty());
    let mut output = Vec::new();

    write_incremental(&mut output, &value, &mut session)
        .expect("incremental encoding should succeed");

    assert_eq!(output, expected);
}

/// Verifies incremental encoding may leave accepted output on a budget error.
#[test]
fn test_write_incremental_preserves_partial_output_on_budget_error() {
    let limits = JsonEncodeLimits::empty()
        .with_output_bytes_limit(ResourceLimit::new(JsonResource::OutputBytes, 4))
        .with_value_limits(JsonValueLimits::empty().with_structure_limits(
            StructureLimits::new().with_nodes_limit(ResourceLimit::new(JsonResource::Nodes, 16)),
        ));
    let mut session = JsonEncodeSession::owned(limits);
    let mut output = Vec::new();

    let error = write_incremental(&mut output, &json!([1, 2, 3]), &mut session)
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
    assert_eq!(session.value_budget().used_nodes(), Some(0));
}

/// Verifies incremental encoding maps destination failures to `Write`.
#[test]
fn test_write_incremental_preserves_partial_output_on_io_error() {
    let mut output = ResourceBudget::new(JsonResource::OutputBytes, 16);
    let mut value = JsonValueBudget::new(JsonValueLimits::empty().with_structure_limits(
        StructureLimits::new().with_nodes_limit(ResourceLimit::new(JsonResource::Nodes, 16)),
    ));
    let mut session = JsonEncodeSession::borrowing_output(&mut output, &mut value);
    let mut writer = PrefixWriter {
        accepted: Vec::new(),
        maximum: 2,
    };

    let error = write_incremental(&mut writer, &[1_u8, 2_u8], &mut session)
        .expect_err("the destination writer must fail incrementally");

    assert!(matches!(error, JsonEncodeError::Write(_)));
    assert_eq!(writer.accepted, b"[1");
    drop(session);
    assert_eq!(output.used(), writer.accepted.len());
    assert_eq!(value.used_nodes(), Some(0));
}

/// Verifies incremental Serde failures retain their accepted prefix and usage.
#[test]
fn test_write_incremental_preserves_partial_output_on_serde_error() {
    let limits = JsonEncodeLimits::empty()
        .with_output_bytes_limit(ResourceLimit::new(JsonResource::OutputBytes, 16))
        .with_value_limits(JsonValueLimits::empty().with_structure_limits(
            StructureLimits::new().with_nodes_limit(ResourceLimit::new(JsonResource::Nodes, 16)),
        ));
    let mut session = JsonEncodeSession::owned(limits);
    let mut output = Vec::new();

    let error = write_incremental(&mut output, &FailsAfterPrefix, &mut session)
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
    assert_eq!(session.value_budget().used_nodes(), Some(0));
}

/// Verifies an incremental panic rolls back value state but retains accepted
/// output.
#[test]
fn test_write_incremental_panic_rolls_back_value_budget() {
    let mut output = ResourceBudget::new(JsonResource::OutputBytes, 16);
    let mut value = JsonValueBudget::new(JsonValueLimits::empty().with_structure_limits(
        StructureLimits::new().with_nodes_limit(ResourceLimit::new(JsonResource::Nodes, 16)),
    ));
    let mut session = JsonEncodeSession::borrowing_output(&mut output, &mut value);
    let mut writer = Vec::new();

    let result = panic::catch_unwind(panic::AssertUnwindSafe(|| {
        write_incremental(&mut writer, &PanicsAfterPrefix, &mut session)
    }));

    assert!(result.is_err());
    drop(session);
    assert_eq!(output.used(), writer.len());
    assert_eq!(value.used_nodes(), Some(0));
}

/// Verifies a session remains usable after an uncommitted writer attempt.
#[test]
fn test_write_buffered_reuses_session_after_failed_attempt() {
    let mut session = JsonEncodeSession::owned(
        JsonEncodeLimits::empty()
            .with_output_bytes_limit(ResourceLimit::new(JsonResource::OutputBytes, 16))
            .with_value_limits(
                JsonValueLimits::empty().with_structure_limits(
                    StructureLimits::new()
                        .with_nodes_limit(ResourceLimit::new(JsonResource::Nodes, 16)),
                ),
            ),
    );
    let mut failed_writer = PrefixWriter {
        accepted: Vec::new(),
        maximum: 2,
    };

    let _ = write_buffered(&mut failed_writer, &[1_u8, 2_u8], &mut session)
        .expect_err("the injected writer failure must drop the value attempt");
    assert_eq!(
        session
            .output_budget()
            .expect("output budget should remain configured")
            .used(),
        failed_writer.accepted.len(),
    );
    assert_eq!(session.value_budget().used_nodes(), Some(0));

    let mut successful_writer = Vec::new();
    write_buffered(&mut successful_writer, &true, &mut session)
        .expect("a later value must commit after the failed attempt");

    assert_eq!(successful_writer, b"true");
    assert_eq!(
        session
            .output_budget()
            .expect("output budget should remain configured")
            .used(),
        failed_writer.accepted.len() + successful_writer.len(),
    );
    assert_eq!(session.value_budget().used_nodes(), Some(1));
}

/// Verifies node accounting stops a long source before it is exhausted.
#[test]
fn test_encode_node_limit_stops_before_source_tail() {
    let serialized = Cell::new(0);
    let value = CountedSequence {
        serialized: &serialized,
        len: 1_000,
    };
    let mut session = JsonTestLimits::new().with_max_nodes(3).encode_session();

    let error =
        encode(&value, &mut session).expect_err("the node budget must reject the long sequence");

    assert!(matches!(error, JsonEncodeError::Budget(_)));
    assert!(serialized.get() < value.len);
}

/// Verifies depth accounting rejects before traversing the complete recursive
/// value.
#[test]
fn test_encode_depth_limit_checks_complete_source_depth() {
    const SOURCE_DEPTH: usize = 128;

    let serialized = Cell::new(0);
    let value = CountedDepth {
        serialized: &serialized,
        remaining: SOURCE_DEPTH - 1,
    };
    let mut session = JsonTestLimits::new().with_max_depth(4).encode_session();

    let error = encode(&value, &mut session)
        .expect_err("the depth budget must reject recursive serialization");

    assert!(matches!(error, JsonEncodeError::Budget(_)));
    assert!(serialized.get() < SOURCE_DEPTH);
}

/// Verifies arbitrary-precision number rejection occurs before a later value.
#[test]
fn test_encode_number_limit_stops_before_source_tail() {
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
        JsonTestLimits::new().with_max_number_bytes(LARGE_NUMBER_TEXT.len() - 1),
        JsonResource::NumberBytes,
        &serialized_tail,
        0,
    );
}

/// Verifies UTF-8 key rejection occurs before a later map entry.
#[test]
fn test_encode_key_limit_stops_before_source_tail() {
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
        0,
    );
}

/// Verifies UTF-8 string rejection occurs before a later sequence value.
#[test]
fn test_encode_string_limit_stops_before_source_tail() {
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
        0,
    );
}

/// Verifies an unknown map rejects at its actual online boundary.
#[test]
fn test_encode_unknown_map_limit_stops_before_source_tail() {
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
        0,
    );
}
