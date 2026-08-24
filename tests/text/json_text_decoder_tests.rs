// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests the stateful strict JSON text decoder public API.

use qubit_budget::ResourceLimit;
use qubit_budget::json::JsonDecodeLimits;
use qubit_budget::json::JsonDecodeSession;
use qubit_budget::json::JsonResource;
use qubit_budget::json::JsonValueLimits;
use qubit_json::decode::JsonDecodeError;
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

/// Verifies a decoder returns a typed value for one complete document.
#[test]
fn test_json_text_decoder_decodes_typed_value() {
    let session = JsonDecodeSession::owned(JsonDecodeLimits::<JsonResource, usize>::builder().build());
    let value = JsonDecoder::new(session)
        .decode_utf8::<bool>(b"true")
        .expect("JSON boolean should decode");

    assert!(value);
}

/// Verifies validation accounts a complete document without deserializing it.
#[test]
fn test_json_text_decoder_validates_complete_document() {
    let session = JsonDecodeSession::owned(JsonDecodeLimits::<JsonResource, usize>::builder().build());
    JsonDecoder::new(session)
        .validate_utf8(br#"{"ok":[true,null]}"#)
        .expect("a complete JSON document should validate");
}

/// Verifies seed deserialization failures retain safe Serde metadata.
#[test]
fn test_json_text_decoder_maps_seed_failure() {
    let session = JsonDecodeSession::owned(JsonDecodeLimits::<JsonResource, usize>::builder().build());
    assert!(
        JsonDecoder::new(session)
            .decode_seed_utf8(FailingSeed, b"true")
            .is_err()
    );
}

/// Verifies a seed that leaves input unread is rejected by the final check.
#[test]
fn test_json_text_decoder_rejects_unconsumed_seed_input() {
    let session = JsonDecodeSession::owned(JsonDecodeLimits::<JsonResource, usize>::builder().build());
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
    let value = JsonDecoder::owned(limits)
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
        decoder.decode_utf8::<Value>(b"-9223372036854775808").unwrap(),
        json!(i64::MIN),
    );
    assert_eq!(
        decoder.decode_utf8::<Value>(b"18446744073709551615").unwrap(),
        json!(u64::MAX),
    );
    for input in [b"-9223372036854775809".as_slice(), b"18446744073709551616".as_slice()] {
        let error = decoder
            .decode_utf8::<Value>(input)
            .expect_err("an integer outside the supported range must fail");
        assert!(matches!(
            error,
            JsonDecodeError::Syntax(error)
                if error.reason() == JsonSyntaxErrorReason::IntegerOutOfRange
        ));
    }
}

/// Verifies non-finite results are rejected for fractional/exponential JSON.
#[test]
fn test_json_text_decoder_rejects_float_outside_finite_f64_range() {
    let error = JsonDecoder::unlimited()
        .decode_utf8::<Value>(b"1e400")
        .expect_err("infinite f64 results must be rejected");
    assert!(matches!(
        error,
        JsonDecodeError::Syntax(error)
            if error.reason() == JsonSyntaxErrorReason::FloatOutOfRange
    ));
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
    let error = JsonDecoder::owned(limits)
        .decode_utf8::<Value>(input)
        .expect_err("the tighter number-byte budget must reject first");
    assert!(matches!(error, JsonDecodeError::Budget(_)));
}
