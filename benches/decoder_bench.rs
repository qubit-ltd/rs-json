// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Criterion benchmarks for public JSON decoding entry points.

use criterion::{
    BenchmarkId,
    Criterion,
    Throughput,
    black_box,
    criterion_group,
    criterion_main,
};
use qubit_json::LenientJsonDecoder;
use serde::Deserialize;

/// Typed value used by constrained-decoder benchmarks.
#[derive(Deserialize)]
struct BenchmarkRecord {
    /// Identifies the benchmark record.
    id: u64,
    /// Stores a representative short text value.
    text: String,
}

/// Runs the public decoder benchmarks over representative input normalization
/// paths.
fn benchmark_decoder(c: &mut Criterion) {
    let decoder = LenientJsonDecoder::default();
    let cases = [
        ("plain", r#"{"id":7,"text":"plain"}"#),
        ("fenced", "```json\n{\"id\":7,\"text\":\"fenced\"}\n```"),
        ("raw-control", "{\"id\":7,\"text\":\"line one\nline two\"}"),
    ];

    for (name, input) in cases {
        let mut group = c.benchmark_group(name);
        group.bench_function("decode", |bencher| {
            bencher.iter(|| {
                consume_record(
                    decoder
                        .decode::<BenchmarkRecord>(black_box(input))
                        .expect("benchmark input must decode"),
                )
            });
        });
        group.bench_function("decode_object", |bencher| {
            bencher.iter(|| {
                consume_record(
                    decoder
                        .decode_object::<BenchmarkRecord>(black_box(input))
                        .expect("benchmark input must decode as an object"),
                )
            });
        });
        group.bench_function("decode_value", |bencher| {
            bencher.iter(|| {
                decoder
                    .decode_value(black_box(input))
                    .expect("benchmark input must decode as a value")
            });
        });
        group.finish();
    }

    let array_input = r#"[{"id":7,"text":"array"}]"#;
    c.bench_function("array/decode_array", |bencher| {
        bencher.iter(|| {
            let records = decoder
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
