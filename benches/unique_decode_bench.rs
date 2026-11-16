// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Criterion latency and throughput benchmarks for unique-key decoding.

mod unique_decode_support;

use std::hint::black_box;

use criterion::BatchSize;
use criterion::Criterion;
use criterion::Throughput;
use criterion::criterion_group;
use criterion::criterion_main;
#[cfg(feature = "jcs")]
use qubit_json::decode::JcsDecoder;
use qubit_json::decode::JsonDecoder;
use qubit_json::decode::JsonNumberPolicy;
use qubit_json::decode::JsonValidatedDecodeOptions;
use qubit_json::value::SafeJsonValue;
use unique_decode_support::decode_path::DecodePath;
use unique_decode_support::decode_path::assert_decoded;
use unique_decode_support::decode_path::assert_duplicate_rejected;
use unique_decode_support::decode_path::decode_batch;
use unique_decode_support::decode_path::decode_once;
use unique_decode_support::decode_path::decode_reject_once;
use unique_decode_support::decode_path::decode_with_decoder;
use unique_decode_support::decode_path::finite_decoder;
use unique_decode_support::fixtures::Shape;
use unique_decode_support::fixtures::fixture;
use unique_decode_support::fixtures::reject_fixture;
use unique_decode_support::lexical_array_fixtures::ArrayFixture;
use unique_decode_support::lexical_array_fixtures::ArrayShape;
use unique_decode_support::lexical_array_fixtures::CHAIN_DEPTHS;
use unique_decode_support::lexical_array_fixtures::RECORD_SIZES;
use unique_decode_support::lexical_array_fixtures::Record;
use unique_decode_support::lexical_array_fixtures::SIZES as ARRAY_SIZES;
use unique_decode_support::structural_decode;
use unique_decode_support::structural_fixtures::COUNTS;
use unique_decode_support::structural_fixtures::DuplicatePosition;
use unique_decode_support::structural_fixtures::StructuralShape;

const SIZES: [usize; 3] = [1024, 64 * 1024, 1024 * 1024];

fn unique_decode(c: &mut Criterion) {
    for shape in Shape::ALL {
        for target_bytes in SIZES {
            let fixture = fixture(shape, target_bytes);
            let size = format!("{}B", fixture.bytes());
            for path in DecodePath::ALL {
                for finite in [false, true] {
                    if path.is_direct_serde() && finite {
                        continue;
                    }
                    let budget = if path.is_direct_serde() {
                        "no_budget"
                    } else if finite {
                        "finite"
                    } else {
                        "unlimited"
                    };
                    let decoded = decode_once(path, &fixture, finite);
                    assert_decoded(&decoded, path, &fixture);
                    black_box(decoded);
                    for decoded in decode_batch(path, &fixture, finite, 8) {
                        assert_decoded(&decoded, path, &fixture);
                        black_box(decoded);
                    }
                    let base = format!("unique_decode/{}/{}/{}/{}", shape.name(), size, path.name(), budget);
                    let mut fresh = c.benchmark_group(format!("{base}/fresh"));
                    fresh.throughput(Throughput::Bytes(fixture.bytes() as u64));
                    fresh.bench_function("decode", |bencher| {
                        bencher.iter(|| {
                            if path.is_direct_serde() {
                                black_box(decode_once(path, &fixture, false));
                            } else {
                                let mut decoder = if finite {
                                    finite_decoder(&fixture, 1)
                                } else {
                                    JsonDecoder::unlimited()
                                };
                                black_box(decode_with_decoder(path, &fixture, &mut decoder));
                            }
                        });
                    });
                    fresh.finish();

                    let mut reused = c.benchmark_group(format!("{base}/batch8"));
                    reused.throughput(Throughput::Bytes((fixture.bytes() * 8) as u64));
                    reused.bench_function("decode", |bencher| {
                        bencher.iter_batched(
                            || {
                                if finite {
                                    finite_decoder(&fixture, 8)
                                } else {
                                    JsonDecoder::unlimited()
                                }
                            },
                            |mut decoder| {
                                for _ in 0..8 {
                                    black_box(decode_with_decoder(path, black_box(&fixture), &mut decoder));
                                }
                            },
                            BatchSize::SmallInput,
                        );
                    });
                    reused.finish();
                }
            }
        }
        for target_bytes in SIZES {
            let reject = reject_fixture(shape, target_bytes);
            for finite in [false, true] {
                let budget = if finite { "finite" } else { "unlimited" };
                let name = format!(
                    "unique_decode_reject/{}/{}/unique_value/{}/fresh",
                    shape.name(),
                    reject.bytes(),
                    budget
                );
                let _ = black_box(assert_duplicate_rejected(decode_reject_once(&reject, finite)));
                let mut group = c.benchmark_group(name);
                group.throughput(Throughput::Bytes(reject.bytes() as u64));
                group.bench_function("duplicate_key", |bencher| {
                    bencher.iter(|| {
                        let _ = black_box(decode_reject_once(black_box(&reject), finite));
                    });
                });
                group.finish();
            }
        }
    }
}

fn early_duplicate_tail(c: &mut Criterion) {
    for tail_bytes in SIZES {
        let input = format!(r#"{{"x":0,"x":1,"tail":"{}"}}"#, "a".repeat(tail_bytes));
        let bytes = input.as_bytes();
        let unique_result =
            JsonDecoder::unlimited().decode_unique_seed_utf8(std::marker::PhantomData::<serde_json::Value>, bytes);
        assert!(unique_result.is_err());
        let mut group = c.benchmark_group(format!("early_duplicate_tail/unique_seed/{tail_bytes}B"));
        group.throughput(Throughput::Bytes(bytes.len() as u64));
        group.bench_function("decode", |bencher| {
            bencher.iter(|| {
                black_box(
                    JsonDecoder::unlimited()
                        .decode_unique_seed_utf8(std::marker::PhantomData::<serde_json::Value>, black_box(bytes)),
                )
            });
        });
        group.finish();

        #[cfg(feature = "jcs")]
        {
            let jcs_result = JcsDecoder::unlimited().decode_utf8(bytes);
            assert!(jcs_result.is_err());
            let mut group = c.benchmark_group(format!("early_duplicate_tail/jcs/{tail_bytes}B"));
            group.throughput(Throughput::Bytes(bytes.len() as u64));
            group.bench_function("decode", |bencher| {
                bencher.iter(|| black_box(JcsDecoder::unlimited().decode_utf8(black_box(bytes))));
            });
            group.finish();
        }
    }
}

fn unique_decode_structural(c: &mut Criterion) {
    for shape in StructuralShape::ALL {
        for count in COUNTS {
            let fixture = unique_decode_support::structural_fixtures::success_fixture(shape, count);
            for path in DecodePath::ALL {
                for finite in [false, true] {
                    if path.is_direct_serde() && finite {
                        continue;
                    }
                    let budget = if path.is_direct_serde() {
                        "no_budget"
                    } else if finite {
                        "finite"
                    } else {
                        "unlimited"
                    };
                    let decoded = structural_decode::decode_once(path, &fixture, finite);
                    structural_decode::assert_decoded(&decoded, &fixture);
                    black_box(decoded);

                    let mut batch_decoder = if finite {
                        structural_decode::finite_decoder(&fixture, 8)
                    } else {
                        JsonDecoder::unlimited()
                    };
                    for decoded in structural_decode::decode_batch_with_decoder(path, &fixture, &mut batch_decoder) {
                        structural_decode::assert_decoded(&decoded, &fixture);
                        black_box(decoded);
                    }

                    let base = format!(
                        "unique_decode_structural/{}/{}items/{}/{budget}",
                        shape.name(),
                        fixture.count,
                        path.name()
                    );
                    let mut fresh = c.benchmark_group(format!("{base}/fresh"));
                    fresh.throughput(Throughput::Bytes(fixture.text.len() as u64));
                    fresh.bench_function("decode", |bencher| {
                        bencher.iter(|| black_box(structural_decode::decode_once(path, &fixture, finite)));
                    });
                    fresh.finish();

                    let mut batch = c.benchmark_group(format!("{base}/batch8"));
                    batch.throughput(Throughput::Bytes((fixture.text.len() * 8) as u64));
                    batch.bench_function("decode", |bencher| {
                        bencher.iter_batched(
                            || {
                                if finite {
                                    structural_decode::finite_decoder(&fixture, 8)
                                } else {
                                    JsonDecoder::unlimited()
                                }
                            },
                            |mut decoder| {
                                let decoded =
                                    structural_decode::decode_batch_with_decoder(path, &fixture, &mut decoder);
                                black_box(decoded);
                            },
                            BatchSize::SmallInput,
                        );
                    });
                    batch.finish();
                }
            }
        }
    }

    for (key_style, fixture) in COUNTS
        .into_iter()
        .map(|count| {
            (
                "short_key",
                unique_decode_support::structural_fixtures::success_fixture(StructuralShape::WideObject, count),
            )
        })
        .chain(COUNTS.into_iter().map(|count| {
            (
                "long_prefix",
                unique_decode_support::structural_fixtures::long_prefix_success_fixture(count),
            )
        }))
    {
        for path in ["owned_value", "validated_preserve"] {
            let decoded = match path {
                "owned_value" => structural_decode::decode_owned_value(&fixture),
                "validated_preserve" => structural_decode::decode_validated_preserve(&fixture),
                _ => unreachable!(),
            };
            structural_decode::assert_decoded(&decoded, &fixture);
            black_box(decoded);
            let name = format!(
                "unique_decode_structural/wide_object/{}items/{key_style}/{path}/unlimited/fresh",
                fixture.count
            );
            let mut group = c.benchmark_group(name);
            group.throughput(Throughput::Bytes(fixture.text.len() as u64));
            group.bench_function("decode", |bencher| {
                bencher.iter(|| {
                    let decoded = match path {
                        "owned_value" => structural_decode::decode_owned_value(black_box(&fixture)),
                        "validated_preserve" => structural_decode::decode_validated_preserve(black_box(&fixture)),
                        _ => unreachable!(),
                    };
                    black_box(decoded)
                });
            });
            group.finish();
        }

        let source: serde_json::Value =
            serde_json::from_str(&fixture.text).expect("projection source fixture must parse");
        assert_eq!(structural_decode::project_value(&source), source);
        let name = format!(
            "unique_decode_structural/wide_object/{}items/{key_style}/value_projection/unlimited/fresh",
            fixture.count
        );
        let mut group = c.benchmark_group(name);
        group.throughput(Throughput::Bytes(fixture.text.len() as u64));
        group.bench_function("project", |bencher| {
            bencher.iter(|| black_box(structural_decode::project_value(black_box(&source))));
        });
        group.finish();
    }

    for count in COUNTS {
        for position in [DuplicatePosition::Early, DuplicatePosition::Late] {
            let fixture = unique_decode_support::structural_fixtures::reject_fixture(count, position);
            for finite in [false, true] {
                let budget = if finite { "finite" } else { "unlimited" };
                structural_decode::assert_reject_fixture(&fixture);
                let error = unique_decode_support::decode_path::assert_duplicate_rejected(
                    structural_decode::decode_reject_once(&fixture, finite),
                );
                let _ = black_box(error);
                let name = format!(
                    "unique_decode_structural_reject/wide_object/{}items/{}/{budget}/fresh",
                    fixture.count,
                    position.name()
                );
                let mut group = c.benchmark_group(name);
                group.throughput(Throughput::Bytes(fixture.text.len() as u64));
                group.bench_function("duplicate_key", |bencher| {
                    bencher.iter(|| {
                        let _ = black_box(structural_decode::decode_reject_once(black_box(&fixture), finite));
                    });
                });
                group.finish();
            }
        }
    }
}

fn lexical_arrays(c: &mut Criterion) {
    for fixture in std::iter::once(ArrayFixture::empty())
        .chain(ARRAY_SIZES.into_iter().map(ArrayFixture::scalars))
        .chain(RECORD_SIZES.into_iter().map(ArrayFixture::records))
        .chain(CHAIN_DEPTHS.into_iter().map(ArrayFixture::chain))
    {
        let mut paths = vec!["value", "validated_preserve"];
        if fixture.shape != ArrayShape::Chain && fixture.shape != ArrayShape::Empty {
            paths.extend(["unique_typed", "unique_seed"]);
        }
        #[cfg(feature = "jcs")]
        paths.push("jcs_value");
        for path in paths {
            for finite in [false, true] {
                if fixture.shape == ArrayShape::Chain && path != "value" && path != "validated_preserve" {
                    continue;
                }
                let budget = if finite { "finite" } else { "unlimited" };
                match path {
                    "value" => assert_lexical_owner(&decode_lexical_value(&fixture, finite), &fixture),
                    "validated_preserve" => assert_lexical_owner(&decode_lexical_preserve(&fixture, finite), &fixture),
                    "unique_typed" if fixture.shape == ArrayShape::Scalar => {
                        let mut decoder = if finite {
                            lexical_finite_decoder(&fixture)
                        } else {
                            JsonDecoder::unlimited()
                        };
                        let value = decoder
                            .decode_unique_str::<Vec<u64>>(&fixture.text)
                            .expect("typed array preflight");
                        assert_eq!(value.len(), fixture.size);
                        assert!(value.iter().enumerate().all(|(index, item)| *item == index as u64));
                    }
                    "unique_seed" if fixture.shape == ArrayShape::Scalar => {
                        let mut decoder = if finite {
                            lexical_finite_decoder(&fixture)
                        } else {
                            JsonDecoder::unlimited()
                        };
                        let value = decoder
                            .decode_unique_seed_utf8(std::marker::PhantomData::<Vec<u64>>, fixture.text.as_bytes())
                            .expect("seed array preflight");
                        assert_eq!(value.len(), fixture.size);
                        assert!(value.iter().enumerate().all(|(index, item)| *item == index as u64));
                    }
                    "unique_typed" | "unique_seed" if fixture.shape == ArrayShape::Records => {
                        let mut decoder = if finite {
                            lexical_finite_decoder(&fixture)
                        } else {
                            JsonDecoder::unlimited()
                        };
                        let value = if path == "unique_typed" {
                            decoder
                                .decode_unique_str::<Vec<Record>>(&fixture.text)
                                .expect("typed record preflight")
                        } else {
                            decoder
                                .decode_unique_seed_utf8(
                                    std::marker::PhantomData::<Vec<Record>>,
                                    fixture.text.as_bytes(),
                                )
                                .expect("seed record preflight")
                        };
                        assert_eq!(value.len(), fixture.size);
                        assert!(
                            value
                                .iter()
                                .enumerate()
                                .all(|(index, row)| row.id == index as u64 && row.label == format!("v{index:06}"))
                        );
                    }
                    #[cfg(feature = "jcs")]
                    "jcs_value" => {
                        let value = if finite {
                            JcsDecoder::with_limits(lexical_limits(&fixture))
                                .decode_value_str(&fixture.text)
                                .expect("JCS preflight")
                        } else {
                            JcsDecoder::unlimited()
                                .decode_value_str(&fixture.text)
                                .expect("JCS preflight")
                        };
                        assert_jcs_lexical_owner(&value, &fixture);
                    }
                    _ => unreachable!("unsupported lexical path preflight"),
                }
                let name = format!(
                    "lexical_arrays/{}/{}/{path}/{budget}",
                    fixture.shape.name(),
                    fixture.size
                );
                let mut group = c.benchmark_group(name);
                group.throughput(Throughput::Bytes(fixture.text.len() as u64));
                group.bench_function("fresh", |bencher| {
                    bencher.iter(|| {
                        match path {
                            "value" => {
                                black_box(decode_lexical_value(black_box(&fixture), finite));
                            }
                            "validated_preserve" => {
                                black_box(decode_lexical_preserve(black_box(&fixture), finite));
                            }
                            "unique_typed" if fixture.shape == ArrayShape::Scalar => {
                                let mut decoder = if finite {
                                    lexical_finite_decoder(&fixture)
                                } else {
                                    JsonDecoder::unlimited()
                                };
                                black_box(
                                    decoder
                                        .decode_unique_str::<Vec<u64>>(black_box(&fixture.text))
                                        .expect("typed array decodes"),
                                );
                            }
                            "unique_seed" if fixture.shape == ArrayShape::Scalar => {
                                let mut decoder = if finite {
                                    lexical_finite_decoder(&fixture)
                                } else {
                                    JsonDecoder::unlimited()
                                };
                                black_box(
                                    decoder
                                        .decode_unique_seed_utf8(
                                            std::marker::PhantomData::<Vec<u64>>,
                                            black_box(fixture.text.as_bytes()),
                                        )
                                        .expect("seed array decodes"),
                                );
                            }
                            "unique_typed" | "unique_seed" if fixture.shape == ArrayShape::Records => {
                                let mut decoder = if finite {
                                    lexical_finite_decoder(&fixture)
                                } else {
                                    JsonDecoder::unlimited()
                                };
                                let result = if path == "unique_typed" {
                                    decoder
                                        .decode_unique_str::<Vec<Record>>(black_box(&fixture.text))
                                        .expect("typed records decode")
                                } else {
                                    decoder
                                        .decode_unique_seed_utf8(
                                            std::marker::PhantomData::<Vec<Record>>,
                                            black_box(fixture.text.as_bytes()),
                                        )
                                        .expect("seed records decode")
                                };
                                black_box(result);
                            }
                            #[cfg(feature = "jcs")]
                            "jcs_value" => {
                                black_box(if finite {
                                    let limits = lexical_limits(&fixture);
                                    JcsDecoder::with_limits(limits)
                                        .decode_value_str(black_box(&fixture.text))
                                        .expect("JCS array decodes")
                                } else {
                                    JcsDecoder::unlimited()
                                        .decode_value_str(black_box(&fixture.text))
                                        .expect("JCS array decodes")
                                });
                            }
                            _ => unreachable!("unsupported lexical array path"),
                        };
                    });
                });
                group.finish();
            }
        }
    }
}

fn lexical_limits(fixture: &ArrayFixture) -> qubit_budget::json::JsonDecodeLimits {
    qubit_budget::json::JsonDecodeLimits::builder()
        .input_bytes_limit(qubit_budget::ResourceLimit::new(
            qubit_budget::json::JsonResource::InputBytes,
            fixture.text.len(),
        ))
        .max_nodes(fixture.nodes)
        .max_depth(fixture.depth)
        .max_sequence_items(fixture.sequence_items)
        .max_payload_bytes(fixture.text.len())
        .build()
}

fn lexical_finite_decoder(fixture: &ArrayFixture) -> JsonDecoder<'static> {
    JsonDecoder::with_limits(lexical_limits(fixture))
}

fn decode_lexical_value(fixture: &ArrayFixture, finite: bool) -> SafeJsonValue {
    let mut decoder = if finite {
        lexical_finite_decoder(fixture)
    } else {
        JsonDecoder::unlimited()
    };
    decoder.decode_value_str(&fixture.text).expect("lexical value decodes")
}

fn decode_lexical_preserve(fixture: &ArrayFixture, finite: bool) -> SafeJsonValue {
    let mut decoder = if finite {
        lexical_finite_decoder(fixture)
    } else {
        JsonDecoder::unlimited()
    };
    decoder
        .decode_unique_validated_str(&fixture.text, &JsonValidatedDecodeOptions::builder().build(), |_, _| {
            Ok::<_, std::convert::Infallible>(JsonNumberPolicy::Preserve)
        })
        .expect("validated preserve decodes")
}

fn assert_lexical_owner(owner: &SafeJsonValue, fixture: &ArrayFixture) {
    if fixture.shape == ArrayShape::Chain {
        let mut current = owner.as_value();
        for _ in 0..fixture.size {
            current = current
                .as_array()
                .expect("chain node is array")
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
fn assert_jcs_lexical_owner(owner: &SafeJsonValue, fixture: &ArrayFixture) {
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

criterion_group!(
    benches,
    unique_decode,
    unique_decode_structural,
    early_duplicate_tail,
    lexical_arrays
);
criterion_main!(benches);
