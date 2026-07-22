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
///
/// # Parameters
///
/// * `c` - Criterion context used to register public decoder benchmarks.
///
/// # Panics
///
/// Panics when a fixed benchmark fixture no longer satisfies its documented
/// decoding contract.
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

    let plain_bytes = plain_input.as_bytes();
    let mut bytes_comparison = c.benchmark_group("plain-bytes-comparison");
    bytes_comparison.bench_function("serde_json_from_slice", |bencher| {
        bencher.iter(|| {
            consume_record(
                serde_json::from_slice::<BenchmarkRecord>(black_box(
                    plain_bytes,
                ))
                .expect("strict benchmark input must decode"),
            )
        });
    });
    bytes_comparison.bench_function("strict_decoder_decode_slice", |bencher| {
        bencher.iter(|| {
            consume_record(
                strict_decoder
                    .decode_slice::<BenchmarkRecord>(black_box(plain_bytes))
                    .expect("strict decoder benchmark input must decode"),
            )
        });
    });
    bytes_comparison.bench_function(
        "default_decoder_decode_slice",
        |bencher| {
            bencher.iter(|| {
                consume_record(
                    default_decoder
                        .decode_slice::<BenchmarkRecord>(black_box(plain_bytes))
                        .expect("default decoder benchmark input must decode"),
                )
            });
        },
    );
    bytes_comparison.finish();

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

/// Runs size-scaling benchmarks that mirror the HTTP and LLM SDK consumers.
///
/// The strict byte benchmarks include both a reused decoder and a decoder
/// constructed inside the measured iteration. The latter mirrors the current
/// `rs-http` call sites, which configure strict decoding immediately before
/// each response or SSE payload is decoded.
///
/// # Parameters
///
/// * `c` - Criterion context used to register downstream-shaped benchmarks.
///
/// # Panics
///
/// Panics when a generated benchmark payload no longer satisfies its expected
/// decoding contract.
fn benchmark_downstream_scaling(c: &mut Criterion) {
    let strict_decoder = LenientJsonDecoder::new(JsonDecodeOptions::strict());
    let default_decoder = LenientJsonDecoder::default();
    let mut plain_group = c.benchmark_group("downstream-plain-bytes");

    for payload_bytes in [1_024_usize, 65_536, 1_048_576] {
        let input = benchmark_record_input(payload_bytes, None);
        plain_group.throughput(Throughput::Bytes(input.len() as u64));
        plain_group.bench_with_input(
            BenchmarkId::new("serde_json_from_slice", payload_bytes),
            &input,
            |bencher, input| {
                bencher.iter(|| {
                    consume_record(
                        serde_json::from_slice::<BenchmarkRecord>(black_box(
                            input.as_bytes(),
                        ))
                        .expect("strict byte benchmark input must decode"),
                    )
                });
            },
        );
        plain_group.bench_with_input(
            BenchmarkId::new("strict_decoder_decode_slice", payload_bytes),
            &input,
            |bencher, input| {
                bencher.iter(|| {
                    consume_record(
                        strict_decoder
                            .decode_slice::<BenchmarkRecord>(black_box(
                                input.as_bytes(),
                            ))
                            .expect(
                                "strict decoder byte benchmark input must decode",
                            ),
                    )
                });
            },
        );
        plain_group.bench_with_input(
            BenchmarkId::new(
                "strict_decoder_construct_and_decode_slice",
                payload_bytes,
            ),
            &input,
            |bencher, input| {
                bencher.iter(|| {
                    let decoder = LenientJsonDecoder::new(
                        JsonDecodeOptions::strict(),
                    );
                    consume_record(
                        decoder
                            .decode_slice::<BenchmarkRecord>(black_box(
                                input.as_bytes(),
                            ))
                            .expect(
                                "strict decoder byte benchmark input must decode",
                            ),
                    )
                });
            },
        );
        plain_group.bench_with_input(
            BenchmarkId::new("default_decoder_decode_slice", payload_bytes),
            &input,
            |bencher, input| {
                bencher.iter(|| {
                    consume_record(
                        default_decoder
                            .decode_slice::<BenchmarkRecord>(black_box(
                                input.as_bytes(),
                            ))
                            .expect(
                                "default decoder byte benchmark input must decode",
                            ),
                    )
                });
            },
        );
    }
    plain_group.finish();

    let mut lenient_group = c.benchmark_group("downstream-lenient-typed");
    for payload_bytes in [1_024_usize, 65_536, 1_048_576] {
        let plain = benchmark_record_input(payload_bytes, None);
        let fenced = format!("```json\n{plain}\n```");
        let pretty = format!(
            "{{\n  \"id\": 7,\n  \"text\": \"{}\"\n}}",
            "a".repeat(payload_bytes),
        );
        let sparse_control = benchmark_record_input(payload_bytes, Some(1_024));
        for (name, input) in [
            ("plain", plain),
            ("fenced", fenced),
            ("pretty", pretty),
            ("sparse-control", sparse_control),
        ] {
            lenient_group.throughput(Throughput::Bytes(input.len() as u64));
            lenient_group.bench_with_input(
                BenchmarkId::new(name, payload_bytes),
                &input,
                |bencher, input| {
                    bencher.iter(|| {
                        consume_record(
                            default_decoder
                                .decode_object::<BenchmarkRecord>(black_box(
                                    input.as_str(),
                                ))
                                .expect(
                                    "lenient typed benchmark input must decode",
                                ),
                        )
                    });
                },
            );
        }
    }
    lenient_group.finish();

    let failure_payload_bytes = 65_536;
    let plain = benchmark_record_input(failure_payload_bytes, None);
    let malformed = &plain[..plain.len() - 1];
    let wrong_top_level = format!("[{plain}]");
    let bounded_decoder = LenientJsonDecoder::new(
        JsonDecodeOptions::strict().with_max_input_bytes(Some(plain.len() - 1)),
    );
    let mut failure_group = c.benchmark_group("downstream-failures");
    failure_group.throughput(Throughput::Bytes(plain.len() as u64));
    failure_group.bench_function("invalid-json", |bencher| {
        bencher.iter(|| {
            black_box(
                default_decoder
                    .decode_object::<BenchmarkRecord>(black_box(malformed))
                    .map_or_else(
                        |error| error,
                        |_| panic!("malformed benchmark input must fail"),
                    ),
            )
        });
    });
    failure_group.bench_function("top-level-mismatch", |bencher| {
        bencher.iter(|| {
            black_box(
                default_decoder
                    .decode_object::<BenchmarkRecord>(black_box(
                        wrong_top_level.as_str(),
                    ))
                    .map_or_else(
                        |error| error,
                        |_| {
                            panic!(
                                "array benchmark input must fail object decoding"
                            )
                        },
                    ),
            )
        });
    });
    failure_group.bench_function("size-limit-rejection", |bencher| {
        bencher.iter(|| {
            black_box(
                bounded_decoder
                    .decode_slice::<BenchmarkRecord>(black_box(
                        plain.as_bytes(),
                    ))
                    .map_or_else(
                        |error| error,
                        |_| panic!("oversized benchmark input must fail"),
                    ),
            )
        });
    });
    for payload_bytes in [65_536_usize, 1_048_576] {
        let plain = benchmark_record_input(payload_bytes, None);
        let first_field_type_error =
            plain.replacen("\"id\":7", "\"id\":\"wrong\"", 1);
        let last_field_type_error = format!(
            "{{\"text\":\"{}\",\"id\":\"wrong\"}}",
            "a".repeat(payload_bytes),
        );

        for (name, input) in [
            ("first-field-type-error", first_field_type_error),
            ("last-field-type-error", last_field_type_error),
        ] {
            failure_group.throughput(Throughput::Bytes(input.len() as u64));
            failure_group.bench_with_input(
                BenchmarkId::new(name, payload_bytes),
                &input,
                |bencher, input| {
                    bencher.iter(|| {
                        black_box(
                            strict_decoder
                                .decode_slice::<BenchmarkRecord>(black_box(input.as_bytes()))
                                .map_or_else(
                                    |error| error,
                                    |_| panic!("type-mismatched benchmark input must fail"),
                                ),
                        )
                    });
                },
            );
        }
    }
    failure_group.finish();
}

/// Runs scaling benchmarks for control-character normalization.
///
/// # Parameters
///
/// * `c` - Criterion context used to register normalization benchmarks.
///
/// # Panics
///
/// Panics when a generated control-character payload cannot be decoded.
fn benchmark_control_character_scaling(c: &mut Criterion) {
    let decoder = LenientJsonDecoder::default();
    let mut group = c.benchmark_group("control-characters");

    for payload_bytes in [1_024_usize, 65_536] {
        for (name, control_stride) in
            [("plain", None), ("sparse", Some(1_024)), ("dense", Some(2))]
        {
            let input = control_character_input(payload_bytes, control_stride);
            let normalized_limit = normalized_control_character_input_bytes(
                input.len(),
                payload_bytes,
                control_stride,
            );
            let bounded_decoder = LenientJsonDecoder::new(
                JsonDecodeOptions::default()
                    .with_max_normalized_bytes(Some(normalized_limit)),
            );
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
            group.bench_with_input(
                BenchmarkId::new(format!("{name}-normalized-limit"), payload_bytes),
                &input,
                |bencher, input| {
                    bencher.iter(|| {
                        black_box(
                            bounded_decoder
                                .decode_value(black_box(input.as_str()))
                                .expect(
                                    "bounded benchmark input must decode as a value",
                                ),
                        )
                    });
                },
            );
        }
    }
    group.finish();
}

/// Returns the exact normalized byte size for a generated control input.
///
/// # Parameters
///
/// * `input_bytes` - Raw byte size of the generated JSON object.
/// * `payload_bytes` - Number of bytes in its JSON string payload.
/// * `control_stride` - Optional spacing between raw NUL characters.
///
/// # Returns
///
/// The repaired JSON byte size, where every raw NUL expands from one byte to
/// the six-byte `\\u0000` escape.
///
/// # Panics
///
/// Panics when `control_stride` is `Some(0)`.
fn normalized_control_character_input_bytes(
    input_bytes: usize,
    payload_bytes: usize,
    control_stride: Option<usize>,
) -> usize {
    let control_count = control_stride.map_or(0, |stride| {
        assert_ne!(stride, 0, "control stride must be nonzero");
        payload_bytes.div_ceil(stride)
    });
    input_bytes + (control_count * 5)
}

/// Builds a JSON object whose string payload has the requested control density.
///
/// # Parameters
///
/// * `payload_bytes` - Number of bytes to place in the object's string field.
/// * `control_stride` - Optional distance between raw NUL characters.
///
/// # Returns
///
/// A JSON-like object accepted by the lenient value decoder.
///
/// # Panics
///
/// Panics when `control_stride` is `Some(0)`.
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

/// Builds a typed JSON object with a requested string payload size and optional
/// raw-control-character density.
///
/// # Parameters
///
/// * `payload_bytes` - Number of bytes to place in the record's text field.
/// * `control_stride` - Optional distance between raw NUL characters.
///
/// # Returns
///
/// A JSON-like object accepted by the lenient typed decoder.
///
/// # Panics
///
/// Panics when `control_stride` is `Some(0)`.
fn benchmark_record_input(
    payload_bytes: usize,
    control_stride: Option<usize>,
) -> String {
    let mut input = String::with_capacity(payload_bytes + 18);
    input.push_str("{\"id\":7,\"text\":\"");
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
///
/// # Parameters
///
/// * `record` - Deserialized benchmark value whose fields are consumed.
fn consume_record(record: BenchmarkRecord) {
    black_box(record.id);
    black_box(record.text);
}

criterion_group!(
    benches,
    benchmark_decoder,
    benchmark_control_character_scaling,
    benchmark_downstream_scaling
);
criterion_main!(benches);
