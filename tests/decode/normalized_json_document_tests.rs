// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Verifies prepared normalized documents and their accounting boundary.

use qubit_budget::ResourceLimit;
use qubit_budget::json::JsonDecodeLimits;
use qubit_budget::json::JsonResource;
use qubit_json::decode::NormalizingJsonDecodePolicy;
use qubit_json::decode::NormalizingJsonDecoder;
use serde::Deserialize;
use serde::de::DeserializeSeed;
use serde::de::Deserializer;
use serde_json::Value;

/// Custom resource identity used to verify facade genericity.
#[derive(Clone, Debug, PartialEq, Eq)]
enum CustomResource {
    /// Raw JSON input bytes.
    InputBytes,
}

/// Seed that requests a string borrowing from the prepared document.
struct BorrowedStrSeed;

impl<'de> DeserializeSeed<'de> for BorrowedStrSeed {
    type Value = &'de str;

    /// Deserializes a string borrowed from the supplied document.
    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        <&str>::deserialize(deserializer)
    }
}

/// Verifies document decoding can borrow through both typed and seed APIs.
#[test]
fn test_normalized_json_document_supports_borrowing_and_seed() {
    let mut decoder = NormalizingJsonDecoder::with_limits(
        NormalizingJsonDecodePolicy::lenient(),
        JsonDecodeLimits::<JsonResource, usize>::default(),
    );
    let document = decoder
        .prepare_str("  \"borrowed\"  ")
        .expect("normalization must succeed");

    let typed: &str = decoder
        .decode_document(&document)
        .expect("typed document decoding must borrow");
    let seeded = decoder
        .decode_document_seed(&document, BorrowedStrSeed)
        .expect("seeded document decoding must borrow");

    assert_eq!(typed, "borrowed");
    assert_eq!(seeded, "borrowed");
    assert_eq!(document.as_str(), "\"borrowed\"");
    assert_eq!(document.raw_input_bytes(), 14);
    assert_eq!(document.normalized_input_bytes(), 10);
}

/// Verifies allocated normalization remains owned by the document and can be
/// decoded through a target that performs JSON unescaping.
#[test]
fn test_normalized_json_document_owns_repaired_text() {
    let mut decoder = NormalizingJsonDecoder::with_limits(
        NormalizingJsonDecodePolicy::lenient(),
        JsonDecodeLimits::<JsonResource, usize>::default(),
    );
    let document = decoder
        .prepare_str("\"line\nfeed\"")
        .expect("control-character escaping must succeed");
    let value: String = decoder
        .decode_document(&document)
        .expect("repaired document must support owned decoding");

    assert_eq!(value, "line\nfeed");
    assert_eq!(document.as_str(), "\"line\\nfeed\"");
}

/// Verifies prepare charges input once and every successful decode commits a
/// distinct value.
#[test]
fn test_normalized_json_document_separates_input_and_value_accounting() {
    let limits = JsonDecodeLimits::builder()
        .max_input_bytes(3_usize)
        .max_normalized_input_bytes(3_usize)
        .max_nodes(2_usize)
        .build();
    let mut decoder =
        NormalizingJsonDecoder::with_limits(NormalizingJsonDecodePolicy::lenient(), limits);
    let document = decoder.prepare_str("\"x\"").expect("prepare must fit");

    let _: &str = decoder
        .decode_document(&document)
        .expect("first decode must fit");
    let _: &str = decoder
        .decode_document(&document)
        .expect("second decode must fit");

    assert_eq!(
        decoder
            .session()
            .input_budget()
            .expect("input budget")
            .used(),
        3
    );
    assert_eq!(
        decoder
            .session()
            .normalized_input_budget()
            .expect("normalized input budget")
            .used(),
        3,
    );
    assert_eq!(decoder.session().value_budget().used_nodes(), Some(2));
}

/// Verifies the normalizing facade preserves custom resource identities and
/// quantity types in its unified error.
#[test]
fn test_normalizing_decoder_supports_custom_resource_and_quantity_types() {
    let limits = JsonDecodeLimits::<CustomResource, u64>::builder()
        .input_bytes_limit(ResourceLimit::new(CustomResource::InputBytes, 2_u64))
        .build();
    let mut decoder =
        NormalizingJsonDecoder::with_limits(NormalizingJsonDecodePolicy::lenient(), limits);

    let error = decoder
        .prepare_str("\"x\"")
        .expect_err("the custom input budget must reject three bytes");

    assert_eq!(
        error.budget_error().expect("budget error").resource(),
        &CustomResource::InputBytes
    );
}

/// Verifies prepared documents retain object, array, and validation parity
/// with the one-shot facade methods.
#[test]
fn test_normalized_json_document_supports_typed_root_checks_and_validation() {
    let mut decoder = NormalizingJsonDecoder::with_limits(
        NormalizingJsonDecodePolicy::lenient(),
        JsonDecodeLimits::<JsonResource, usize>::default(),
    );
    let object = decoder
        .prepare_str(" {\"value\":1} ")
        .expect("object prepare");
    let array = decoder.prepare_str(" [1,2] ").expect("array prepare");

    let object_value: Value = decoder
        .decode_object_document(&object)
        .expect("object document decode");
    let array_value: Vec<u8> = decoder
        .decode_array_document(&array)
        .expect("array document decode");
    decoder
        .validate_document(&object)
        .expect("prepared document validation");

    assert_eq!(object_value["value"], 1);
    assert_eq!(array_value, [1, 2]);
}

/// Verifies a failed prepared-document materialization preserves its already
/// committed input usage while rolling back all staged value usage.
#[test]
fn test_normalized_json_document_failure_rolls_back_only_value_usage() {
    let input = r#"{"flag":1}"#;
    let limits = JsonDecodeLimits::builder()
        .max_input_bytes(input.len())
        .max_normalized_input_bytes(input.len())
        .max_nodes(2_usize)
        .build();
    let mut decoder =
        NormalizingJsonDecoder::with_limits(NormalizingJsonDecodePolicy::lenient(), limits);
    let document = decoder.prepare_str(input).expect("prepare must succeed");

    let _ = decoder
        .decode_document::<std::collections::HashMap<String, bool>>(&document)
        .expect_err("number must not deserialize as bool");

    assert_eq!(
        decoder
            .session()
            .input_budget()
            .expect("input budget")
            .used(),
        input.len()
    );
    assert_eq!(
        decoder
            .session()
            .normalized_input_budget()
            .expect("normalized input budget")
            .used(),
        input.len(),
    );
    assert_eq!(decoder.session().value_budget().used_nodes(), Some(0));
}
