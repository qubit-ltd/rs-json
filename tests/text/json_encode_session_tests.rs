// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests reusable JSON encode sessions.

use std::io;
use std::io::Write;
use std::panic::AssertUnwindSafe;
use std::panic::catch_unwind;

use qubit_budget::ResourceBudget;
use qubit_budget::ResourceLimit;
use qubit_budget::StructureLimits;
use qubit_budget::json::JsonEncodeLimits;
use qubit_budget::json::JsonEncodeSession;
use qubit_budget::json::JsonMeasurement;
use qubit_budget::json::JsonResource;
use qubit_budget::json::JsonValueBudget;
use qubit_budget::json::JsonValueLimits;
use serde::Serialize;
use serde::Serializer;
use serde::ser::Error as SerError;
use serde::ser::SerializeSeq;

use crate::text::json_encode_test_support::encode;
use crate::text::json_encode_test_support::write_buffered;
use crate::text::json_encode_test_support::write_incremental;

/// Value that serializes one accepted prefix before returning an error.
struct FailsAfterPrefix;

impl Serialize for FailsAfterPrefix {
    /// Emits one sequence item, then returns a custom serialization error.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut sequence = serializer.serialize_seq(Some(2))?;
        sequence.serialize_element(&1_u8)?;
        Err(SerError::custom("intentional streaming failure"))
    }
}

/// Value that serializes one accepted prefix before unwinding.
struct PanicsAfterPrefix;

impl Serialize for PanicsAfterPrefix {
    /// Emits one sequence item, then panics during serialization.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut sequence = serializer.serialize_seq(Some(2))?;
        sequence.serialize_element(&1_u8)?;
        panic!("intentional streaming panic");
    }
}

/// Writer that accepts a bounded prefix, then fails every later write.
struct PrefixWriter {
    /// Bytes accepted before the configured failure boundary.
    accepted: Vec<u8>,
    /// Maximum number of bytes that may be accepted.
    maximum: usize,
}

impl Write for PrefixWriter {
    /// Accepts only the remaining prefix capacity before returning an I/O
    /// error.
    fn write(&mut self, input: &[u8]) -> io::Result<usize> {
        if self.accepted.len() == self.maximum {
            return Err(io::Error::other("intentional writer failure"));
        }
        let accepted = (self.maximum - self.accepted.len()).min(input.len());
        self.accepted.extend_from_slice(&input[..accepted]);
        Ok(accepted)
    }

    /// Does not add buffering beyond the accepted prefix.
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Verifies encode sessions expose their output-byte resource.
/// Verifies borrowed encode sessions charge caller-owned budgets in place.
#[test]
fn test_encode_session_exposes_only_output_resource() {
    let encode = JsonEncodeSession::owned(
        JsonEncodeLimits::empty().with_output_bytes_limit(ResourceLimit::new(
            JsonResource::OutputBytes,
            8,
        )),
    );

    assert_eq!(encode.max_output_bytes(), Some(8));
}

/// Verifies output-byte consumption is cumulative and atomic within one
/// attempt.
#[test]
fn test_encode_attempt_consumes_output_bytes_atomically() {
    let mut session = JsonEncodeSession::owned(
        JsonEncodeLimits::empty().with_output_bytes_limit(ResourceLimit::new(
            JsonResource::OutputBytes,
            3,
        )),
    );

    let mut attempt = session.begin_value();
    attempt
        .try_consume_output_bytes(3)
        .expect("exact output fits");
    let error = attempt
        .try_consume_output_bytes(1)
        .expect_err("output budget is exhausted");
    assert_eq!(error.resource(), &JsonResource::OutputBytes);
}

/// Verifies encode attempts preserve every embedded JSON value limit.
#[test]
fn test_encode_attempt_preserves_embedded_value_limits() {
    let value_limits = JsonValueLimits::empty()
        .with_string_bytes_limit(ResourceLimit::new(
            JsonResource::StringBytes,
            2,
        ))
        .with_payload_bytes_limit(ResourceLimit::new(
            JsonResource::PayloadBytes,
            3,
        ))
        .with_structure_limits(
            StructureLimits::empty()
                .with_nodes_limit(ResourceLimit::new(JsonResource::Nodes, 2)),
        );
    let mut session = JsonEncodeSession::owned(
        JsonEncodeLimits::empty().with_value_limits(value_limits),
    );

    let mut attempt = session.begin_value();
    attempt
        .try_admit(JsonMeasurement::String { depth: 1, bytes: 2 })
        .expect("exact string limit fits");
    assert_eq!(
        attempt
            .try_admit(JsonMeasurement::String { depth: 1, bytes: 3 })
            .expect_err("overlong string fails")
            .resource(),
        &JsonResource::StringBytes,
    );
    attempt
        .try_admit(JsonMeasurement::Key { bytes: 1 })
        .expect("exact payload limit fits");
    assert_eq!(
        attempt
            .try_admit(JsonMeasurement::Key { bytes: 1 })
            .expect_err("exhausted payload fails")
            .resource(),
        &JsonResource::PayloadBytes,
    );
    attempt
        .try_admit(JsonMeasurement::Null { depth: 1 })
        .expect("exact node limit fits");
    assert_eq!(
        attempt
            .try_admit(JsonMeasurement::Null { depth: 1 })
            .expect_err("exhausted node limit fails")
            .resource(),
        &JsonResource::Nodes,
    );
}

#[test]
fn test_encode_session_borrows_output_and_value_budgets() {
    let mut output = ResourceBudget::new(JsonResource::OutputBytes, 32);
    let mut value = JsonValueBudget::new(
        JsonValueLimits::empty().with_structure_limits(
            StructureLimits::empty()
                .with_nodes_limit(ResourceLimit::new(JsonResource::Nodes, 16)),
        ),
    );
    let mut session =
        JsonEncodeSession::borrowing_output(&mut output, &mut value);

    let encoded = encode(&serde_json::json!({"name": "qubit"}), &mut session)
        .expect("borrowed budgets should support online encoding");

    assert_eq!(encoded, br#"{"name":"qubit"}"#);
    assert_eq!(output.used(), encoded.len());
    assert!(
        value
            .used_nodes()
            .expect("nodes limit should be configured")
            > 0
    );
}

/// Verifies buffered vector serialization publishes neither output nor value
/// accounting when a streamed Serde value fails.
#[test]
fn test_encode_stream_failure_rolls_back_output_and_value() {
    let mut session = JsonEncodeSession::owned(
        JsonEncodeLimits::empty()
            .with_max_output_bytes(64)
            .with_max_nodes(2),
    );

    assert!(encode(&FailsAfterPrefix, &mut session).is_err());
    assert_eq!(session.output_budget().expect("output budget").used(), 0,);
    assert_eq!(session.value_budget().used_nodes(), Some(0));
}

/// Verifies buffered writes retain their externally accepted prefix and output
/// charge, while their staged value accounting rolls back after I/O failure.
#[test]
fn test_buffered_writer_failure_keeps_prefix_and_rolls_back_value() {
    let mut session = JsonEncodeSession::owned(
        JsonEncodeLimits::empty()
            .with_max_output_bytes(64)
            .with_max_nodes(3),
    );
    let mut writer = PrefixWriter {
        accepted: Vec::new(),
        maximum: 2,
    };

    assert!(write_buffered(&mut writer, &[1_u8, 2_u8], &mut session).is_err());
    assert_eq!(writer.accepted.len(), 2);
    assert_eq!(
        session.output_budget().expect("output budget").used(),
        writer.accepted.len(),
    );
    assert_eq!(session.value_budget().used_nodes(), Some(0));
}

/// Verifies incremental streaming retains its accepted output prefix but drops
/// staged values when serialization returns an error.
#[test]
fn test_incremental_stream_failure_keeps_prefix_and_rolls_back_value() {
    let mut session = JsonEncodeSession::owned(
        JsonEncodeLimits::empty()
            .with_max_output_bytes(64)
            .with_max_nodes(2),
    );
    let mut writer = Vec::new();

    assert!(
        write_incremental(&mut writer, &FailsAfterPrefix, &mut session)
            .is_err()
    );
    assert!(!writer.is_empty());
    assert_eq!(
        session.output_budget().expect("output budget").used(),
        writer.len(),
    );
    assert_eq!(session.value_budget().used_nodes(), Some(0));
}

/// Verifies panic unwind keeps incremental output side effects but rolls back
/// staged values so the session can encode a later value.
#[test]
fn test_incremental_panic_keeps_prefix_and_reuses_value_capacity() {
    let mut session = JsonEncodeSession::owned(
        JsonEncodeLimits::empty()
            .with_max_output_bytes(64)
            .with_max_nodes(1),
    );
    let mut writer = Vec::new();

    let result = catch_unwind(AssertUnwindSafe(|| {
        write_incremental(&mut writer, &PanicsAfterPrefix, &mut session)
    }));

    assert!(result.is_err());
    assert!(!writer.is_empty());
    assert_eq!(
        session.output_budget().expect("output budget").used(),
        writer.len(),
    );
    assert_eq!(session.value_budget().used_nodes(), Some(0));
    encode(&true, &mut session)
        .expect("panic must leave value capacity for the next encode");
    assert_eq!(session.value_budget().used_nodes(), Some(1));
}
