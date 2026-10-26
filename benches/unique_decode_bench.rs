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
use qubit_json::decode::JsonDecoder;
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
                    assert_decoded(&decoded, path, shape);
                    black_box(decoded);
                    for decoded in decode_batch(path, &fixture, finite, 8) {
                        assert_decoded(&decoded, path, shape);
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
                let mut group = c.benchmark_group(name);
                group.throughput(Throughput::Bytes(reject.bytes() as u64));
                group.bench_function("duplicate_key", |bencher| {
                    bencher.iter(|| {
                        let _ = black_box(assert_duplicate_rejected(decode_reject_once(
                            black_box(&reject),
                            finite,
                        )));
                    });
                });
                group.finish();
            }
        }
    }
}

criterion_group!(benches, unique_decode);
criterion_main!(benches);
