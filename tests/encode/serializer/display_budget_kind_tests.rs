// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests display-based JSON budget categories.

use std::collections::BTreeMap;
use std::fmt;

use qubit_budget::ResourceLimit;
use qubit_budget::json::JsonEncodeLimits;
use qubit_budget::json::JsonEncodeSession;
use qubit_budget::json::JsonResource;
use qubit_json::encode::JsonEncodeErrorKind;
use serde::Serialize;
use serde::Serializer;

use crate::encode::json_encode_test_support::encode;

/// Wraps a displayable value for Serde string collection tests.
struct DisplayValue<T>(T);

impl<T: fmt::Display> Serialize for DisplayValue<T> {
    /// Collects the wrapped value's display representation as a JSON string.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_str(&self.0)
    }
}

/// Displays a string in separate UTF-8 chunks for preview accounting tests.
struct ChunkedDisplay;

impl fmt::Display for ChunkedDisplay {
    /// Writes two multi-byte segments in separate formatter calls.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("é")?;
        formatter.write_str("🙂")
    }
}

/// Serializes ChunkedDisplay through Serde's collected-string path.
struct ChunkedValue;

impl Serialize for ChunkedValue {
    /// Delegates serialization to the multi-chunk display fixture.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_str(&ChunkedDisplay)
    }
}

/// Supplies displayed text as an ordered map key in budget tests.
#[derive(Eq, Ord, PartialEq, PartialOrd)]
struct DisplayKey(&'static str);

impl Serialize for DisplayKey {
    /// Emits the key text through Serde's display collection hook.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_str(&self.0)
    }
}

/// Verifies display collection is rejected when its string budget is zero.
#[test]
fn test_display_budget_kind_checks_collected_string() {
    let limits = JsonEncodeLimits::<JsonResource, usize>::builder()
        .max_string_bytes(0)
        .build();
    let mut session = JsonEncodeSession::from_limits(limits);

    let error = encode(&DisplayValue("display"), &mut session).expect_err("display text exceeds the string budget");

    assert_eq!(error.kind(), JsonEncodeErrorKind::Budget);
    assert_eq!(
        error
            .budget_error()
            .expect("budget failures expose their measured source")
            .resource(),
        &JsonResource::StringBytes,
    );
}

/// Verifies previews for multiple UTF-8 chunks do not charge staged usage.
#[test]
fn test_display_preview_charges_string_once_after_collection() {
    let limits = JsonEncodeLimits::<JsonResource, usize>::builder()
        .max_nodes(1)
        .max_payload_bytes(6)
        .build();
    let mut session = JsonEncodeSession::from_limits(limits);

    let output = encode(&ChunkedValue, &mut session).expect("both chunks fit the complete string budget");

    assert_eq!(output, "\"é🙂\"".as_bytes());
    assert_eq!(session.value_budget().used_nodes(), Some(1));
    assert_eq!(session.value_budget().used_payload_bytes(), Some(6));
}

/// Verifies an empty displayed string still admits one string node.
#[test]
fn test_display_preview_admits_empty_string_once() {
    let limits = JsonEncodeLimits::<JsonResource, usize>::builder()
        .max_nodes(1)
        .max_payload_bytes(0)
        .build();
    let mut session = JsonEncodeSession::from_limits(limits);

    let output = encode(&DisplayValue(""), &mut session).expect("empty text fits zero payload bytes");

    assert_eq!(output, b"\"\"");
    assert_eq!(session.value_budget().used_nodes(), Some(1));
    assert_eq!(session.value_budget().used_payload_bytes(), Some(0));
}

/// Verifies successive display values preview against already staged payload.
#[test]
fn test_display_preview_uses_remaining_payload_capacity() {
    let values = (DisplayValue("abc"), DisplayValue("de"));
    let limits = JsonEncodeLimits::<JsonResource, usize>::builder()
        .max_nodes(3)
        .max_payload_bytes(5)
        .build();
    let mut session = JsonEncodeSession::from_limits(limits);

    encode(&values, &mut session).expect("the combined display payload fits");

    assert_eq!(session.value_budget().used_nodes(), Some(3));
    assert_eq!(session.value_budget().used_payload_bytes(), Some(5));
}

/// Verifies a prospective UTF-8 string is rejected before the failed chunk is
/// accepted.
#[test]
fn test_display_preview_rejects_prospective_utf8_overflow() {
    let limits = JsonEncodeLimits::<JsonResource, usize>::builder()
        .max_nodes(1)
        .max_string_bytes(5)
        .build();
    let mut session = JsonEncodeSession::from_limits(limits);

    let error = encode(&ChunkedValue, &mut session).expect_err("six UTF-8 bytes exceed the five-byte limit");

    assert_eq!(error.kind(), JsonEncodeErrorKind::Budget);
    assert_eq!(
        error.budget_error().expect("budget error is retained").resource(),
        &JsonResource::StringBytes,
    );
    assert_eq!(session.value_budget().used_nodes(), Some(0));
}

/// Verifies collected map keys use key bytes and are charged once.
#[test]
fn test_display_preview_checks_map_key_bytes_once() {
    let values = BTreeMap::from([(DisplayKey("éx"), true)]);
    let limits = JsonEncodeLimits::<JsonResource, usize>::builder()
        .max_nodes(2)
        .max_key_bytes(3)
        .build();
    let mut session = JsonEncodeSession::from_limits(limits);

    let output = encode(&values, &mut session).expect("three UTF-8 key bytes fit the key budget");

    assert_eq!(output, "{\"éx\":true}".as_bytes());
    assert_eq!(session.value_budget().used_nodes(), Some(2));
}

/// Verifies the output lower bound is checked for each cumulative chunk.
#[test]
fn test_display_preview_checks_output_lower_bound_before_growth() {
    let limits = JsonEncodeLimits::<JsonResource, usize>::builder()
        .output_bytes_limit(ResourceLimit::new(JsonResource::OutputBytes, 2))
        .build();
    let mut session = JsonEncodeSession::from_limits(limits);

    let error = encode(&ChunkedValue, &mut session).expect_err("the three-byte payload exceeds output capacity");

    assert_eq!(error.kind(), JsonEncodeErrorKind::Budget);
    assert_eq!(
        error.budget_error().expect("budget error is retained").resource(),
        &JsonResource::OutputBytes,
    );
}

/// Verifies display payload conversion errors remain typed for narrow
/// quantities.
#[test]
fn test_display_preview_preserves_narrow_quantity_error() {
    let limits = JsonEncodeLimits::<JsonResource, u8>::builder()
        .max_payload_bytes(u8::MAX)
        .build();
    let mut session = JsonEncodeSession::from_limits(limits);

    let error = encode(&DisplayValue("x".repeat(256)), &mut session)
        .expect_err("the test payload must exceed narrow quantity capacity");

    assert_eq!(error.kind(), JsonEncodeErrorKind::Budget);
    assert_eq!(
        error
            .budget_error()
            .expect("quantity conversion is retained")
            .resource(),
        &JsonResource::PayloadBytes,
    );
}
