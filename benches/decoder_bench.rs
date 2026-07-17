// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Criterion benchmarks for public JSON decoding entry points.

mod internal;

use criterion::{
    BenchmarkId,
    Criterion,
    Throughput,
    black_box,
    criterion_group,
    criterion_main,
};
use qubit_json::{
    JsonDecodeOptions,
    LenientJsonDecoder,
};

use internal::BenchmarkRecord;

/// Runs the public decoder benchmarks over representative input normalization
/// paths.
fn benchmark_decoder(c: &mut Criterion) {
    let default_decoder = LenientJsonDecoder::default();
    let strict_decoder = LenientJsonDecoder::new(JsonDecodeOptions::strict());
    let plain_input = r#"{"id":7,"text":"plain"}"#;
    let mut comparison = c.benchmark_group("plain-comparison");
    comparison.bench_function("serde_json", |bencher| {
        bencher.iter(|| {
            consume_record(
                serde_json::from_str::<BenchmarkRecord>(black_box(plain_input))
                    .expect("strict benchmark input must decode"),
            )
        });
    });
    comparison.bench_function("strict_decoder", |bencher| {
        bencher.iter(|| {
            consume_record(
                strict_decoder
                    .decode::<BenchmarkRecord>(black_box(plain_input))
                    .expect("strict decoder benchmark input must decode"),
            )
        });
    });
    comparison.bench_function("default_decoder", |bencher| {
        bencher.iter(|| {
            consume_record(
                default_decoder
                    .decode::<BenchmarkRecord>(black_box(plain_input))
                    .expect("default decoder benchmark input must decode"),
            )
        });
    });
    comparison.finish();

    let cases = [
        ("plain", plain_input),
        ("fenced", "```json\n{\"id\":7,\"text\":\"fenced\"}\n```"),
        ("raw-control", "{\"id\":7,\"text\":\"line one\nline two\"}"),
    ];

    for (name, input) in cases {
        let mut group = c.benchmark_group(name);
        group.bench_function("decode", |bencher| {
            bencher.iter(|| {
                consume_record(
                    default_decoder
                        .decode::<BenchmarkRecord>(black_box(input))
                        .expect("benchmark input must decode"),
                )
            });
        });
        group.bench_function("decode_object", |bencher| {
            bencher.iter(|| {
                consume_record(
                    default_decoder
                        .decode_object::<BenchmarkRecord>(black_box(input))
                        .expect("benchmark input must decode as an object"),
                )
            });
        });
        group.bench_function("decode_value", |bencher| {
            bencher.iter(|| {
                default_decoder
                    .decode_value(black_box(input))
                    .expect("benchmark input must decode as a value")
            });
        });
        group.finish();
    }

    let array_input = r#"[{"id":7,"text":"array"}]"#;
    c.bench_function("array/decode_array", |bencher| {
        bencher.iter(|| {
            let records = default_decoder
                .decode_array::<BenchmarkRecord>(black_box(array_input))
                .expect("benchmark input must decode as an array");
            for record in records {
                consume_record(record);
            }
        });
    });
}

/// Runs scaling benchmarks for control-character normalization.
fn benchmark_control_character_scaling(c: &mut Criterion) {
    let decoder = LenientJsonDecoder::default();
    let mut group = c.benchmark_group("control-characters");

    for payload_bytes in [1_024_usize, 65_536] {
        for (name, control_stride) in
            [("plain", None), ("sparse", Some(1_024)), ("dense", Some(2))]
        {
            let input = control_character_input(payload_bytes, control_stride);
            group.throughput(Throughput::Bytes(input.len() as u64));
            group.bench_with_input(
                BenchmarkId::new(name, payload_bytes),
                &input,
                |bencher, input| {
                    bencher.iter(|| {
                        black_box(
                            decoder
                                .decode_value(black_box(input.as_str()))
                                .expect(
                                    "benchmark input must decode as a value",
                                ),
                        )
                    });
                },
            );
        }
    }
    group.finish();
}

/// Builds a JSON object whose string payload has the requested control density.
fn control_character_input(
    payload_bytes: usize,
    control_stride: Option<usize>,
) -> String {
    let mut input = String::with_capacity(payload_bytes + 11);
    input.push_str("{\"text\":\"");
    for index in 0..payload_bytes {
        if control_stride.is_some_and(|stride| index % stride == 0) {
            input.push('\u{0000}');
        } else {
            input.push('a');
        }
    }
    input.push_str("\"}");
    input
}

/// Consumes deserialized fields so the benchmark exercises the complete result.
fn consume_record(record: BenchmarkRecord) {
    black_box(record.id);
    black_box(record.text);
}

criterion_group!(
    benches,
    benchmark_decoder,
    benchmark_control_character_scaling
);
criterion_main!(benches);
