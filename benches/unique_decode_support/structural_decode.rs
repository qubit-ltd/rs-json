// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Structural fixture dispatch, bounded decoders, and complete result checks.

use std::collections::BTreeMap;
use std::marker::PhantomData;

use qubit_budget::ResourceLimit;
use qubit_budget::json::JsonDecodeLimits;
use qubit_budget::json::JsonResource;
use qubit_json::decode::JsonDecoder;
use qubit_json::decode::JsonNumberPolicy;
use qubit_json::decode::JsonValidatedDecodeOptions;
use qubit_json::value::DuplicateKeyRejectingJsonValue;
use qubit_json::value::JsonValueEncoder;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde::de::DeserializeSeed;

use super::decode_path::DecodePath;
use super::structural_fixtures::DuplicatePosition;
use super::structural_fixtures::StructuralExpected;
use super::structural_fixtures::StructuralFixture;
use super::structural_fixtures::StructuralRecord;
use super::structural_fixtures::StructuralShape;

#[derive(Debug)]
#[allow(dead_code)] // Keep the decoded allocation alive through benchmark measurement.
pub(crate) enum StructuralDecoded {
    Value(serde_json::Value),
    Object(BTreeMap<String, String>),
    Records(Vec<StructuralRecord>),
}

struct TypedSeed<T>(PhantomData<fn() -> T>);

impl<T> Default for TypedSeed<T> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<'de, T: Deserialize<'de>> DeserializeSeed<'de> for TypedSeed<T> {
    type Value = T;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        T::deserialize(deserializer)
    }
}

pub(crate) fn finite_decoder(fixture: &StructuralFixture, batch: usize) -> JsonDecoder<'static> {
    let multiplier = batch.max(1);
    let limits = JsonDecodeLimits::builder()
        .input_bytes_limit(ResourceLimit::new(
            JsonResource::InputBytes,
            fixture.text.len() * multiplier,
        ))
        .max_nodes(fixture.node_count * multiplier)
        .max_depth(fixture.max_depth)
        .max_sequence_items(fixture.max_sequence_items)
        .max_map_entries(fixture.max_map_entries)
        .max_key_bytes(fixture.max_key_bytes)
        .max_string_bytes(fixture.max_string_bytes)
        .max_number_bytes(fixture.max_number_bytes)
        .max_payload_bytes(fixture.payload_bytes * multiplier)
        .build();
    JsonDecoder::with_limits(limits)
}

fn decode_typed<T: DeserializeOwned>(
    path: DecodePath,
    fixture: &StructuralFixture,
    decoder: &mut JsonDecoder<'static>,
) -> T {
    match path {
        DecodePath::UniqueTyped => decoder
            .decode_unique_str::<T>(&fixture.text)
            .expect("unique typed structural fixture must decode"),
        DecodePath::UniqueSeed => decoder
            .decode_unique_seed_utf8(TypedSeed::<T>::default(), fixture.text.as_bytes())
            .expect("unique seed structural fixture must decode"),
        DecodePath::OrdinaryTyped => decoder
            .decode_str::<T>(&fixture.text)
            .expect("ordinary typed structural fixture must decode"),
        DecodePath::SerdeTyped => serde_json::from_str::<T>(&fixture.text).expect("serde typed fixture must decode"),
        _ => unreachable!("typed dispatch requires a typed path"),
    }
}

fn decode_direct_serde(path: DecodePath, fixture: &StructuralFixture) -> StructuralDecoded {
    match (path, fixture.shape) {
        (DecodePath::SerdeValue, _) => StructuralDecoded::Value(
            serde_json::from_str(&fixture.text).expect("serde value structural fixture must decode"),
        ),
        (DecodePath::SerdeTyped, StructuralShape::WideObject) => StructuralDecoded::Object(
            serde_json::from_str::<BTreeMap<String, String>>(&fixture.text)
                .expect("serde typed object fixture must decode"),
        ),
        (DecodePath::SerdeTyped, StructuralShape::RecordArray) => StructuralDecoded::Records(
            serde_json::from_str::<Vec<StructuralRecord>>(&fixture.text)
                .expect("serde typed record fixture must decode"),
        ),
        _ => unreachable!("direct serde helper requires a serde path"),
    }
}

pub(crate) fn decode_with_decoder(
    path: DecodePath,
    fixture: &StructuralFixture,
    decoder: &mut JsonDecoder<'static>,
) -> StructuralDecoded {
    if path.is_direct_serde() {
        return decode_direct_serde(path, fixture);
    }
    match (path, fixture.shape) {
        (DecodePath::UniqueValue, _) => StructuralDecoded::Value(
            decoder
                .decode_unique_str::<DuplicateKeyRejectingJsonValue>(&fixture.text)
                .expect("unique value structural fixture must decode")
                .into_inner(),
        ),
        (DecodePath::OrdinaryValue, _) => StructuralDecoded::Value(
            decoder
                .decode_str::<serde_json::Value>(&fixture.text)
                .expect("ordinary value structural fixture must decode"),
        ),
        (DecodePath::UniqueTyped | DecodePath::UniqueSeed | DecodePath::OrdinaryTyped, StructuralShape::WideObject) => {
            StructuralDecoded::Object(decode_typed::<BTreeMap<String, String>>(path, fixture, decoder))
        }
        (
            DecodePath::UniqueTyped | DecodePath::UniqueSeed | DecodePath::OrdinaryTyped,
            StructuralShape::RecordArray,
        ) => StructuralDecoded::Records(decode_typed::<Vec<StructuralRecord>>(path, fixture, decoder)),
        (DecodePath::SerdeTyped | DecodePath::SerdeValue, _) => {
            unreachable!("direct serde returned above")
        }
    }
}

pub(crate) fn decode_once(path: DecodePath, fixture: &StructuralFixture, finite: bool) -> StructuralDecoded {
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

pub(crate) fn decode_batch_with_decoder(
    path: DecodePath,
    fixture: &StructuralFixture,
    decoder: &mut JsonDecoder<'static>,
) -> [StructuralDecoded; 8] {
    std::array::from_fn(|_| decode_with_decoder(path, fixture, decoder))
}

pub(crate) fn assert_decoded(decoded: &StructuralDecoded, fixture: &StructuralFixture) {
    let expected = fixture
        .expected
        .as_ref()
        .expect("successful fixture has expected content");
    match (decoded, expected) {
        (StructuralDecoded::Value(actual), StructuralExpected::Object(want)) => {
            let actual = actual.as_object().expect("wide object decodes to object Value");
            assert_eq!(actual.len(), want.len());
            for (key, value) in want {
                assert_eq!(actual.get(key), Some(&serde_json::Value::String(value.clone())));
            }
        }
        (StructuralDecoded::Value(actual), StructuralExpected::Records(want)) => {
            let actual = actual.as_array().expect("record array decodes to array Value");
            assert_eq!(actual.len(), want.len());
            for (value, record) in actual.iter().zip(want) {
                let object = value.as_object().expect("record Value must be an object");
                assert_eq!(object.len(), 2);
                assert_eq!(object.get("id"), Some(&serde_json::Value::from(record.id)));
                assert_eq!(
                    object.get("payload").and_then(serde_json::Value::as_str),
                    Some(record.payload.as_str())
                );
            }
        }
        (StructuralDecoded::Object(actual), StructuralExpected::Object(want)) => {
            assert_eq!(actual, want)
        }
        (StructuralDecoded::Records(actual), StructuralExpected::Records(want)) => {
            assert_eq!(actual, want)
        }
        _ => panic!("decoded structural target must match its full expected value and shape"),
    }
}

pub(crate) fn decode_reject_once(
    fixture: &StructuralFixture,
    finite: bool,
) -> Result<DuplicateKeyRejectingJsonValue, qubit_json::decode::JsonDecodeError> {
    let mut decoder = if finite {
        finite_decoder(fixture, 1)
    } else {
        JsonDecoder::unlimited()
    };
    decoder.decode_unique_str::<DuplicateKeyRejectingJsonValue>(&fixture.text)
}

pub(crate) fn assert_reject_fixture(fixture: &StructuralFixture) {
    assert_eq!(fixture.shape, StructuralShape::WideObject);
    assert!(fixture.expected.is_none());
    assert!(matches!(
        fixture.reject_position,
        Some(DuplicatePosition::Early | DuplicatePosition::Late)
    ));
}

#[allow(dead_code)]
pub(crate) fn decode_owned_value(fixture: &StructuralFixture) -> StructuralDecoded {
    let value = JsonDecoder::unlimited()
        .decode_value_str(&fixture.text)
        .expect("owned value fixture must decode");
    StructuralDecoded::Value(value.into_value())
}

#[allow(dead_code)]
pub(crate) fn decode_validated_preserve(fixture: &StructuralFixture) -> StructuralDecoded {
    let options = JsonValidatedDecodeOptions::builder().build();
    let value = JsonDecoder::unlimited()
        .decode_unique_validated_str(&fixture.text, &options, |_, _| {
            Ok::<_, std::convert::Infallible>(JsonNumberPolicy::Preserve)
        })
        .expect("validated preserve fixture must decode");
    StructuralDecoded::Value(value.into_value())
}

#[allow(dead_code)]
pub(crate) fn project_value(source: &serde_json::Value) -> serde_json::Value {
    JsonValueEncoder::new()
        .encode(source)
        .expect("structural value projection must succeed")
}
