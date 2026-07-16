// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Criterion benchmarks for public JSON decoding entry points.

use criterion::{
    Criterion,
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

/// Consumes deserialized fields so the benchmark exercises the complete result.
fn consume_record(record: BenchmarkRecord) {
    black_box(record.id);
    black_box(record.text);
}

criterion_group!(benches, benchmark_decoder);
criterion_main!(benches);
