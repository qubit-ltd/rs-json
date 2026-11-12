// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Requested-allocation and live-result diagnostics for lexical array decoding.

#[path = "unique_decode_support/allocation_counter.rs"]
mod allocation_counter;
#[allow(dead_code)]
mod unique_decode_support;

use qubit_budget::ResourceLimit;
use qubit_budget::json::JsonDecodeLimits;
use qubit_budget::json::JsonResource;
#[cfg(feature = "jcs")]
use qubit_json::decode::JcsDecoder;
use qubit_json::decode::JsonDecoder;
use qubit_json::decode::JsonNumberPolicy;
use qubit_json::decode::JsonValidatedDecodeOptions;
use qubit_json::value::SafeJsonValue;
use serde::de::DeserializeOwned;
use serde::de::DeserializeSeed;
use unique_decode_support::lexical_array_fixtures::ArrayFixture;
use unique_decode_support::lexical_array_fixtures::ArrayShape;
use unique_decode_support::lexical_array_fixtures::CHAIN_DEPTHS;
use unique_decode_support::lexical_array_fixtures::RECORD_SIZES;
use unique_decode_support::lexical_array_fixtures::Record;
use unique_decode_support::lexical_array_fixtures::SIZES;

#[derive(Clone, Copy)]
enum Path {
    Value,
    ValidatedPreserve,
    Typed,
    Seed,
    #[cfg(feature = "jcs")]
    JcsValue,
}

impl Path {
    const fn name(self) -> &'static str {
        match self {
            Self::Value => "decode_value",
            Self::ValidatedPreserve => "validated_preserve",
            Self::Typed => "unique_typed",
            Self::Seed => "unique_seed",
            #[cfg(feature = "jcs")]
            Self::JcsValue => "jcs_decode_value",
        }
    }
}

fn decoder(fixture: &ArrayFixture, finite: bool) -> JsonDecoder<'static> {
    if !finite {
        return JsonDecoder::unlimited();
    }
    let limits = JsonDecodeLimits::builder()
        .input_bytes_limit(ResourceLimit::new(JsonResource::InputBytes, fixture.text.len()))
        .max_nodes(fixture.nodes)
        .max_depth(fixture.depth)
        .max_sequence_items(fixture.sequence_items)
        .max_payload_bytes(fixture.text.len())
        .build();
    JsonDecoder::with_limits(limits)
}

fn decode_typed<T: DeserializeOwned>(path: Path, fixture: &ArrayFixture, finite: bool) -> T {
    let mut decoder = decoder(fixture, finite);
    match path {
        Path::Typed => decoder
            .decode_unique_str::<T>(&fixture.text)
            .expect("typed array fixture decodes"),
        Path::Seed => decoder
            .decode_unique_seed_utf8(TypedSeed::<T>::default(), fixture.text.as_bytes())
            .expect("seed array fixture decodes"),
        _ => unreachable!("typed decoder used for a non-typed path"),
    }
}

struct TypedSeed<T>(std::marker::PhantomData<fn() -> T>);

impl<T> Default for TypedSeed<T> {
    fn default() -> Self {
        Self(std::marker::PhantomData)
    }
}

impl<'de, T: serde::Deserialize<'de>> DeserializeSeed<'de> for TypedSeed<T> {
    type Value = T;
    fn deserialize<D: serde::Deserializer<'de>>(self, deserializer: D) -> Result<T, D::Error> {
        T::deserialize(deserializer)
    }
}

fn decode_safe(path: Path, fixture: &ArrayFixture, finite: bool) -> SafeJsonValue {
    match path {
        Path::Value => decoder(fixture, finite)
            .decode_value_str(&fixture.text)
            .expect("value fixture decodes"),
        Path::ValidatedPreserve => decoder(fixture, finite)
            .decode_unique_validated_str(&fixture.text, &JsonValidatedDecodeOptions::builder().build(), |_, _| {
                Ok::<_, std::convert::Infallible>(JsonNumberPolicy::Preserve)
            })
            .expect("validated fixture decodes"),
        #[cfg(feature = "jcs")]
        Path::JcsValue => {
            if finite {
                let limits = JsonDecodeLimits::builder()
                    .input_bytes_limit(ResourceLimit::new(JsonResource::InputBytes, fixture.text.len()))
                    .max_nodes(fixture.nodes)
                    .max_depth(fixture.depth)
                    .max_sequence_items(fixture.sequence_items)
                    .max_payload_bytes(fixture.text.len())
                    .build();
                JcsDecoder::with_limits(limits)
                    .decode_value_str(&fixture.text)
                    .expect("JCS fixture decodes")
            } else {
                JcsDecoder::unlimited()
                    .decode_value_str(&fixture.text)
                    .expect("JCS fixture decodes")
            }
        }
        Path::Typed | Path::Seed => unreachable!("typed result does not use SafeJsonValue"),
    }
}

fn shape_fixtures() -> Vec<ArrayFixture> {
    std::iter::once(ArrayFixture::empty())
        .chain(SIZES.into_iter().map(ArrayFixture::scalars))
        .chain(RECORD_SIZES.into_iter().map(ArrayFixture::records))
        .chain(CHAIN_DEPTHS.into_iter().map(ArrayFixture::chain))
        .collect()
}

fn features() -> String {
    std::env::var("RSJSON_FEATURES").unwrap_or_else(|_| {
        if cfg!(feature = "jcs") {
            "jcs".to_owned()
        } else {
            "default".to_owned()
        }
    })
}

fn report(fixture: &ArrayFixture, path: Path, finite: bool) {
    let budget = if finite { "finite" } else { "unlimited" };
    match path {
        Path::Typed | Path::Seed if fixture.shape == ArrayShape::Scalar => {
            let warm = decode_typed::<Vec<u64>>(path, fixture, finite);
            assert_eq!(warm.len(), fixture.size);
            assert!(warm.iter().enumerate().all(|(i, v)| *v == i as u64));
        }
        Path::Typed | Path::Seed if fixture.shape == ArrayShape::Records => {
            let warm = decode_typed::<Vec<Record>>(path, fixture, finite);
            assert_eq!(warm.len(), fixture.size);
            assert!(
                warm.iter()
                    .enumerate()
                    .all(|(i, row)| row.id == i as u64 && row.label == format!("v{i:06}"))
            );
        }
        Path::Typed | Path::Seed => {}
        #[cfg(feature = "jcs")]
        Path::JcsValue => assert_jcs_owner_matches(&decode_safe(path, fixture, finite), fixture),
        _ => assert_owner_matches(&decode_safe(path, fixture, finite), fixture),
    }
    match path {
        Path::Typed | Path::Seed if fixture.shape == ArrayShape::Scalar => {
            let (value, measurement) = allocation_counter::measure(|| decode_typed::<Vec<u64>>(path, fixture, finite));
            assert_eq!(value.len(), fixture.size);
            assert!(value.iter().enumerate().all(|(i, v)| *v == i as u64));
            let delta = measurement.end_live_bytes as isize - measurement.baseline_live_bytes as isize;
            println!(
                "LEXICAL_ARRAY_ALLOC,{},{},{},{},{},{},{},{},{},{},{}",
                fixture.shape.name(),
                fixture.size,
                fixture.text.len(),
                path.name(),
                budget,
                features(),
                measurement.allocation_count,
                measurement.allocated_request_bytes,
                measurement.reallocation_count,
                measurement.peak_extra_bytes,
                delta
            );
            std::hint::black_box(value);
        }
        Path::Typed | Path::Seed if fixture.shape == ArrayShape::Records => {
            let (value, measurement) =
                allocation_counter::measure(|| decode_typed::<Vec<Record>>(path, fixture, finite));
            assert_eq!(value.len(), fixture.size);
            assert!(
                value
                    .iter()
                    .enumerate()
                    .all(|(i, row)| row.id == i as u64 && row.label == format!("v{i:06}"))
            );
            let delta = measurement.end_live_bytes as isize - measurement.baseline_live_bytes as isize;
            println!(
                "LEXICAL_ARRAY_ALLOC,{},{},{},{},{},{},{},{},{},{},{}",
                fixture.shape.name(),
                fixture.size,
                fixture.text.len(),
                path.name(),
                budget,
                features(),
                measurement.allocation_count,
                measurement.allocated_request_bytes,
                measurement.reallocation_count,
                measurement.peak_extra_bytes,
                delta
            );
            std::hint::black_box(value);
        }
        Path::Typed | Path::Seed => {}
        #[cfg(feature = "jcs")]
        Path::JcsValue => {
            let (owner, measurement) = allocation_counter::measure(|| decode_safe(path, fixture, finite));
            assert_jcs_owner_matches(&owner, fixture);
            let delta = measurement.end_live_bytes as isize - measurement.baseline_live_bytes as isize;
            println!(
                "LEXICAL_ARRAY_ALLOC,{},{},{},{},{},{},{},{},{},{},{}",
                fixture.shape.name(),
                fixture.size,
                fixture.text.len(),
                path.name(),
                budget,
                features(),
                measurement.allocation_count,
                measurement.allocated_request_bytes,
                measurement.reallocation_count,
                measurement.peak_extra_bytes,
                delta
            );
            std::hint::black_box(owner);
        }
        _ => {
            let (owner, measurement) = allocation_counter::measure(|| decode_safe(path, fixture, finite));
            assert_owner_matches(&owner, fixture);
            let delta = measurement.end_live_bytes as isize - measurement.baseline_live_bytes as isize;
            println!(
                "LEXICAL_ARRAY_ALLOC,{},{},{},{},{},{},{},{},{},{},{}",
                fixture.shape.name(),
                fixture.size,
                fixture.text.len(),
                path.name(),
                budget,
                features(),
                measurement.allocation_count,
                measurement.allocated_request_bytes,
                measurement.reallocation_count,
                measurement.peak_extra_bytes,
                delta
            );
            std::hint::black_box(owner);
        }
    }
}

fn assert_owner_matches(owner: &SafeJsonValue, fixture: &ArrayFixture) {
    if fixture.shape == ArrayShape::Chain {
        let mut current = owner.as_value();
        for _ in 0..fixture.size {
            current = current
                .as_array()
                .expect("chain node is an array")
                .first()
                .expect("chain child exists");
        }
        assert_eq!(current.as_u64(), Some(0));
    } else {
        assert_eq!(
            owner.as_value(),
            &serde_json::from_str::<serde_json::Value>(&fixture.text).expect("fixture parses")
        );
    }
}

#[cfg(feature = "jcs")]
fn assert_jcs_owner_matches(owner: &SafeJsonValue, fixture: &ArrayFixture) {
    let value = owner.as_value();
    match fixture.shape {
        ArrayShape::Empty => assert!(value.as_array().expect("empty fixture is array").is_empty()),
        ArrayShape::Scalar => {
            let values = value.as_array().expect("scalar fixture is array");
            assert_eq!(values.len(), fixture.size);
            assert!(
                values
                    .iter()
                    .enumerate()
                    .all(|(i, item)| item.as_f64() == Some(i as f64))
            );
        }
        ArrayShape::Records => {
            let rows = value.as_array().expect("record fixture is array");
            assert_eq!(rows.len(), fixture.size);
            assert!(rows.iter().enumerate().all(|(i, row)| {
                let object = row.as_object().expect("record fixture item is object");
                object.len() == 2
                    && object.get("id").and_then(serde_json::Value::as_f64) == Some(i as f64)
                    && object.get("label").and_then(serde_json::Value::as_str) == Some(format!("v{i:06}").as_str())
            }));
        }
        ArrayShape::Chain => {
            let mut current = value;
            for _ in 0..fixture.size {
                current = current
                    .as_array()
                    .expect("chain node is array")
                    .first()
                    .expect("chain child exists");
            }
            assert_eq!(current.as_f64(), Some(0.0));
        }
    }
}

fn main() {
    if std::env::args().any(|argument| argument == "--allocation-counter-self-check") {
        allocation_counter::self_check();
        println!("allocation counter self-check passed");
        return;
    }
    println!(
        "LEXICAL_ARRAY_ALLOC,shape,size,input_bytes,path,budget,features,allocations,requested_bytes,reallocations,peak_extra,end_delta"
    );
    for fixture in shape_fixtures() {
        let mut paths = vec![Path::Value, Path::ValidatedPreserve];
        #[cfg(feature = "jcs")]
        paths.push(Path::JcsValue);
        if matches!(fixture.shape, ArrayShape::Scalar | ArrayShape::Records) {
            paths.extend([Path::Typed, Path::Seed]);
        }
        for path in paths {
            for finite in [false, true] {
                report(&fixture, path, finite);
            }
        }
    }
}
