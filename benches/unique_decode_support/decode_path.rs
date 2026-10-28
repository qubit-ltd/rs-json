// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Decoder dispatch and budget construction shared by benchmark programs.

use std::hint::black_box;
use std::marker::PhantomData;

use qubit_budget::ResourceLimit;
use qubit_budget::json::JsonDecodeLimits;
use qubit_budget::json::JsonResource;
use qubit_json::decode::JsonDecoder;
use qubit_json::decode::JsonMaterializationErrorReason;
use qubit_json::value::DuplicateKeyRejectingJsonValue;
use serde::Deserialize;
use serde::de::DeserializeSeed;
use serde_json::Value;
use serde_json::from_str;

use super::fixtures::Config;
use super::fixtures::Expected;
use super::fixtures::Fixture;
use super::fixtures::Metadata;
use super::fixtures::Shape;
use super::fixtures::WireEnvelope;

#[derive(Debug)]
#[allow(dead_code)] // The owned payload must survive until allocation counters stop.
pub(crate) enum Decoded {
    UniqueValue(serde_json::Value),
    Value(serde_json::Value),
    Config(Config),
    Metadata(Metadata),
    WireEnvelope(WireEnvelope),
}

pub(crate) fn assert_decoded(decoded: &Decoded, path: DecodePath, fixture: &Fixture) {
    let target_matches = match path {
        DecodePath::UniqueValue => matches!(decoded, Decoded::UniqueValue(_)),
        DecodePath::OrdinaryValue | DecodePath::SerdeValue => matches!(decoded, Decoded::Value(_)),
        DecodePath::UniqueTyped | DecodePath::UniqueSeed | DecodePath::OrdinaryTyped | DecodePath::SerdeTyped => {
            matches!(
                (fixture.shape, decoded),
                (Shape::Config, Decoded::Config(_))
                    | (Shape::Metadata, Decoded::Metadata(_))
                    | (Shape::WireEnvelope, Decoded::WireEnvelope(_))
            )
        }
    };
    assert!(target_matches, "decoder path must produce the expected owned target");
    let expected = fixture
        .expected
        .as_ref()
        .expect("success fixture must have expected data");
    match (decoded, expected) {
        (Decoded::Config(actual), Expected::Config(expected)) => assert_eq!(actual, expected),
        (Decoded::Metadata(actual), Expected::Metadata(expected)) => assert_eq!(actual, expected),
        (Decoded::WireEnvelope(actual), Expected::WireEnvelope(expected)) => assert_eq!(actual, expected),
        (Decoded::Value(actual) | Decoded::UniqueValue(actual), _) => {
            let expected: Value = from_str(&fixture.text).expect("successful source fixture must parse");
            assert_eq!(actual, &expected);
        }
        _ => panic!("decoded target must match the full expected fixture"),
    }
}

pub(crate) fn assert_duplicate_rejected(
    result: Result<DuplicateKeyRejectingJsonValue, qubit_json::decode::JsonDecodeError>,
) -> qubit_json::decode::JsonDecodeError {
    let error = result.expect_err("duplicate-key rejection fixture must fail");
    assert_eq!(
        error.materialization_error_reason(),
        Some(JsonMaterializationErrorReason::DuplicateKey)
    );
    error
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DecodePath {
    UniqueValue,
    UniqueTyped,
    UniqueSeed,
    OrdinaryTyped,
    SerdeTyped,
    OrdinaryValue,
    SerdeValue,
}

impl DecodePath {
    pub(crate) const ALL: [Self; 7] = [
        Self::UniqueValue,
        Self::UniqueTyped,
        Self::UniqueSeed,
        Self::OrdinaryTyped,
        Self::SerdeTyped,
        Self::OrdinaryValue,
        Self::SerdeValue,
    ];

    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::UniqueValue => "unique_value",
            Self::UniqueTyped => "unique_typed",
            Self::UniqueSeed => "unique_seed",
            Self::OrdinaryTyped => "ordinary_typed",
            Self::SerdeTyped => "serde_typed",
            Self::OrdinaryValue => "ordinary_value",
            Self::SerdeValue => "serde_value",
        }
    }

    pub(crate) const fn is_direct_serde(self) -> bool {
        matches!(self, Self::SerdeTyped | Self::SerdeValue)
    }
}

pub(crate) fn finite_decoder(fixture: &Fixture, batch: usize) -> JsonDecoder<'static> {
    let multiplier = batch.max(1);
    let bytes = fixture.bytes().saturating_mul(multiplier);
    let nodes = fixture.node_count.saturating_mul(multiplier);
    let payload = fixture.payload_bytes.saturating_mul(multiplier);
    let limits = JsonDecodeLimits::builder()
        .input_bytes_limit(ResourceLimit::new(JsonResource::InputBytes, bytes))
        .max_nodes(nodes)
        .max_depth(fixture.max_depth)
        .max_sequence_items(fixture.max_sequence_items)
        .max_map_entries(fixture.max_map_entries)
        .max_key_bytes(fixture.max_key_bytes)
        .max_string_bytes(fixture.max_string_bytes)
        .max_number_bytes(fixture.max_number_bytes)
        .max_payload_bytes(payload)
        .build();
    JsonDecoder::with_limits(limits)
}

struct TypedSeed<T>(PhantomData<fn() -> T>);

impl<T> Default for TypedSeed<T> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<'de, T> DeserializeSeed<'de> for TypedSeed<T>
where
    T: Deserialize<'de>,
{
    type Value = T;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        T::deserialize(deserializer)
    }
}

pub(crate) fn decode_once(path: DecodePath, fixture: &Fixture, finite: bool) -> Decoded {
    if path.is_direct_serde() {
        return decode_direct_serde(path, fixture);
    }
    let mut decoder = if finite {
        finite_decoder(fixture, 1)
    } else {
        JsonDecoder::unlimited()
    };
    decode_with_decoder(path, fixture, &mut decoder)
}

pub(crate) fn decode_reject_once(
    fixture: &Fixture,
    finite: bool,
) -> Result<DuplicateKeyRejectingJsonValue, qubit_json::decode::JsonDecodeError> {
    let mut decoder = if finite {
        finite_decoder(fixture, 1)
    } else {
        JsonDecoder::unlimited()
    };
    decoder.decode_unique_str::<DuplicateKeyRejectingJsonValue>(&fixture.text)
}

#[allow(dead_code)] // Used by the Criterion target for setup-free batch preflight.
pub(crate) fn decode_batch(path: DecodePath, fixture: &Fixture, finite: bool, batch: usize) -> [Decoded; 8] {
    if path.is_direct_serde() {
        return std::array::from_fn(|_| decode_direct_serde(path, fixture));
    }
    let mut decoder = if finite {
        finite_decoder(fixture, batch)
    } else {
        JsonDecoder::unlimited()
    };
    decode_batch_with_decoder(path, fixture, &mut decoder, batch)
}

pub(crate) fn decode_batch_with_decoder(
    path: DecodePath,
    fixture: &Fixture,
    decoder: &mut JsonDecoder<'static>,
    batch: usize,
) -> [Decoded; 8] {
    assert_eq!(batch, 8, "reused benchmark batches contain exactly eight inputs");
    std::array::from_fn(|_| decode_with_decoder(path, fixture, decoder))
}

pub(crate) fn decode_with_decoder(path: DecodePath, fixture: &Fixture, decoder: &mut JsonDecoder<'static>) -> Decoded {
    if path.is_direct_serde() {
        return decode_direct_serde(path, fixture);
    }
    let input = black_box(fixture.text.as_str());
    match (path, fixture.shape) {
        (DecodePath::UniqueValue, _) => Decoded::UniqueValue(
            decoder
                .decode_unique_str::<DuplicateKeyRejectingJsonValue>(input)
                .expect("unique value fixture must decode")
                .into_inner(),
        ),
        (DecodePath::OrdinaryValue, _) => Decoded::Value(
            decoder
                .decode_str::<serde_json::Value>(input)
                .expect("ordinary value fixture must decode"),
        ),
        (DecodePath::SerdeValue, _) => {
            Decoded::Value(serde_json::from_str::<serde_json::Value>(input).expect("serde value fixture must decode"))
        }
        (DecodePath::UniqueTyped, Shape::Config) => Decoded::Config(
            decoder
                .decode_unique_str::<Config>(input)
                .expect("unique config fixture must decode"),
        ),
        (DecodePath::UniqueTyped, Shape::Metadata) => Decoded::Metadata(
            decoder
                .decode_unique_str::<Metadata>(input)
                .expect("unique metadata fixture must decode"),
        ),
        (DecodePath::UniqueTyped, Shape::WireEnvelope) => Decoded::WireEnvelope(
            decoder
                .decode_unique_str::<WireEnvelope>(input)
                .expect("unique wire fixture must decode"),
        ),
        (DecodePath::UniqueSeed, Shape::Config) => Decoded::Config(
            decoder
                .decode_unique_seed_utf8(TypedSeed::<Config>::default(), input.as_bytes())
                .expect("unique config seed must decode"),
        ),
        (DecodePath::UniqueSeed, Shape::Metadata) => Decoded::Metadata(
            decoder
                .decode_unique_seed_utf8(TypedSeed::<Metadata>::default(), input.as_bytes())
                .expect("unique metadata seed must decode"),
        ),
        (DecodePath::UniqueSeed, Shape::WireEnvelope) => Decoded::WireEnvelope(
            decoder
                .decode_unique_seed_utf8(TypedSeed::<WireEnvelope>::default(), input.as_bytes())
                .expect("unique wire seed must decode"),
        ),
        (DecodePath::OrdinaryTyped, Shape::Config) => Decoded::Config(
            decoder
                .decode_str::<Config>(input)
                .expect("ordinary config fixture must decode"),
        ),
        (DecodePath::OrdinaryTyped, Shape::Metadata) => Decoded::Metadata(
            decoder
                .decode_str::<Metadata>(input)
                .expect("ordinary metadata fixture must decode"),
        ),
        (DecodePath::OrdinaryTyped, Shape::WireEnvelope) => Decoded::WireEnvelope(
            decoder
                .decode_str::<WireEnvelope>(input)
                .expect("ordinary wire fixture must decode"),
        ),
        (DecodePath::SerdeTyped, Shape::Config) => {
            Decoded::Config(serde_json::from_str::<Config>(input).expect("serde config fixture must decode"))
        }
        (DecodePath::SerdeTyped, Shape::Metadata) => {
            Decoded::Metadata(serde_json::from_str::<Metadata>(input).expect("serde metadata fixture must decode"))
        }
        (DecodePath::SerdeTyped, Shape::WireEnvelope) => {
            Decoded::WireEnvelope(serde_json::from_str::<WireEnvelope>(input).expect("serde wire fixture must decode"))
        }
    }
}

fn decode_direct_serde(path: DecodePath, fixture: &Fixture) -> Decoded {
    let input = black_box(fixture.text.as_str());
    match (path, fixture.shape) {
        (DecodePath::SerdeValue, _) => {
            Decoded::Value(serde_json::from_str::<serde_json::Value>(input).expect("serde value fixture must decode"))
        }
        (DecodePath::SerdeTyped, Shape::Config) => {
            Decoded::Config(serde_json::from_str::<Config>(input).expect("serde config fixture must decode"))
        }
        (DecodePath::SerdeTyped, Shape::Metadata) => {
            Decoded::Metadata(serde_json::from_str::<Metadata>(input).expect("serde metadata fixture must decode"))
        }
        (DecodePath::SerdeTyped, Shape::WireEnvelope) => {
            Decoded::WireEnvelope(serde_json::from_str::<WireEnvelope>(input).expect("serde wire fixture must decode"))
        }
        _ => panic!("direct serde helper only accepts serde decode paths"),
    }
}
