// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Criterion benchmarks for budget-aware JSON decoding and encoding.

use std::hint::black_box;

use criterion::BenchmarkId;
use criterion::Criterion;
use criterion::Throughput;
use criterion::criterion_group;
use criterion::criterion_main;
use qubit_budget::json::JsonDecodeLimits;
use qubit_budget::json::JsonDecodeSession;
use qubit_budget::json::JsonEncodeLimits;
use qubit_budget::json::JsonEncodeSession;
use qubit_budget::json::JsonResource;
use qubit_budget::json::JsonValueBudget;
use qubit_budget::json::JsonValueLimits;
use qubit_json::decode::JsonDecoder;
use qubit_json::decode::MarkdownFencePolicy;
use qubit_json::decode::NormalizingJsonDecodePolicy;
use qubit_json::decode::NormalizingJsonDecoder;
use qubit_json::encode::JsonEncoder;
use serde::Deserialize;
use serde::Serialize;
use serde_json::value::RawValue;

mod internal;

use internal::BenchmarkRecord;

const DOCUMENT_SIZES: [usize; 3] = [1_024, 65_536, 1_048_576];
const RECORD: &[u8] = br#"{"id":7,"text":"benchmark-record"}"#;
const DOCUMENT_PREFIX: &[u8] = br#"{"records":["#;
const DOCUMENT_SUFFIX: &[u8] = br#"]}"#;

/// Creates a normalization policy that performs no text rewriting.
fn no_normalization_policy() -> NormalizingJsonDecodePolicy {
    NormalizingJsonDecodePolicy::builder()
        .trim_whitespace(false)
        .strip_utf8_bom(false)
        .markdown_fence_policy(MarkdownFencePolicy::Disabled)
        .escape_control_chars_in_strings(false)
        .build()
}

/// JSON fixture with an object array that scales to each target document size.
#[derive(Debug, Deserialize, Serialize)]
struct Fixture {
    /// Records exercised by the benchmark document.
    records: Vec<BenchmarkRecord>,
}

/// Builds a fixed JSON object-array document close to `target_bytes`.
///
/// # Parameters
///
/// * `target_bytes` - Requested approximate byte length for the document.
///
/// # Returns
///
/// A valid JSON document whose length is no greater than `target_bytes` and
/// differs by less than one encoded record.
///
/// # Panics
///
/// Panics when `target_bytes` cannot hold the fixed document structure or the
/// generated document falls outside the documented size tolerance.
fn benchmark_document(target_bytes: usize) -> Vec<u8> {
    assert!(
        target_bytes >= DOCUMENT_PREFIX.len() + RECORD.len() + DOCUMENT_SUFFIX.len(),
        "benchmark target must hold one record",
    );

    let mut document = Vec::with_capacity(target_bytes);
    document.extend_from_slice(DOCUMENT_PREFIX);
    while document.len() + RECORD.len() + DOCUMENT_SUFFIX.len() <= target_bytes {
        if document.len() > DOCUMENT_PREFIX.len() {
            document.push(b',');
        }
        document.extend_from_slice(RECORD);
    }
    document.extend_from_slice(DOCUMENT_SUFFIX);

    assert!(
        document.len() <= target_bytes && target_bytes - document.len() <= RECORD.len(),
        "benchmark document must remain within one record of its target",
    );
    document
}

/// Registers strict and lenient decoding benchmarks for every document size.
///
/// # Parameters
///
/// * `criterion` - Criterion context used to register the benchmark group.
///
/// # Panics
///
/// Panics when a generated document cannot be decoded by a benchmark path.
fn decode(criterion: &mut Criterion) {
    let mut decoder = NormalizingJsonDecoder::with_limits(
        no_normalization_policy(),
        qubit_budget::json::JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::default(),
    );
    let mut group = criterion.benchmark_group("budgeted-json-decode");

    for target_bytes in DOCUMENT_SIZES {
        let document = benchmark_document(target_bytes);
        let size = document.len();
        let input = std::str::from_utf8(&document).expect("benchmark document must be valid UTF-8");
        let fixture = serde_json::from_slice::<Fixture>(&document).expect("benchmark document must decode");
        black_box(fixture.records.len());
        group.throughput(Throughput::Bytes(document.len() as u64));

        group.bench_with_input(
            BenchmarkId::new("decode/serde_json", size),
            &document,
            |bencher, input| {
                bencher.iter(|| {
                    black_box(serde_json::from_slice::<Fixture>(black_box(input)).expect("fixture must decode"))
                });
            },
        );
        group.bench_with_input(
            BenchmarkId::new("decode/owned-session", size),
            &document,
            |bencher, input| {
                bencher.iter(|| {
                    let session = JsonDecodeSession::from_limits(
                        JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::builder().build(),
                    );
                    black_box(
                        JsonDecoder::new(session)
                            .decode_utf8::<Fixture>(black_box(input))
                            .expect("fixture must decode"),
                    )
                });
            },
        );
        group.bench_with_input(
            BenchmarkId::new("decode/borrowed-session", size),
            &document,
            |bencher, input| {
                bencher.iter(|| {
                    let mut value = JsonValueBudget::new(JsonValueLimits::<JsonResource, usize>::builder().build());
                    let session = JsonDecodeSession::borrowing_value(&mut value);
                    black_box(
                        JsonDecoder::new(session)
                            .decode_utf8::<Fixture>(black_box(input))
                            .expect("fixture must decode"),
                    )
                });
            },
        );
        let reused_session = JsonDecodeSession::from_limits(
            JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::builder().build(),
        );
        let mut reused_decoder = JsonDecoder::new(reused_session);
        group.bench_with_input(
            BenchmarkId::new("decode/reused-session", size),
            &document,
            |bencher, input| {
                bencher.iter(|| {
                    black_box(
                        reused_decoder
                            .decode_utf8::<Fixture>(black_box(input))
                            .expect("fixture must decode"),
                    )
                });
            },
        );
        group.bench_with_input(
            BenchmarkId::new("decode/value-bounded-session", size),
            &document,
            |bencher, input| {
                bencher.iter(|| {
                    let limits = JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::builder()
                        .max_nodes(usize::MAX)
                        .build();
                    black_box(
                        JsonDecoder::with_limits(limits)
                            .decode_utf8::<Fixture>(black_box(input))
                            .expect("fixture must fit the value budget"),
                    )
                });
            },
        );
        group.bench_with_input(
            BenchmarkId::new("decode/lenient-json-decoder", size),
            &input,
            |bencher, input| {
                bencher.iter(|| {
                    black_box(
                        decoder
                            .decode_str::<Fixture>(black_box(input))
                            .expect("fixture must decode"),
                    )
                });
            },
        );
        group.bench_with_input(
            BenchmarkId::new("decode/lenient-json-decoder-with-session", size),
            &input,
            |bencher, input| {
                bencher.iter(|| {
                    black_box(
                        NormalizingJsonDecoder::new(
                            decoder.policy().clone(),
                            JsonDecodeSession::from_limits(
                                JsonDecodeLimits::<qubit_budget::json::JsonResource, usize>::builder().build(),
                            ),
                        )
                        .decode_str::<Fixture>(black_box(input))
                        .expect("fixture must decode"),
                    )
                });
            },
        );
    }
    group.finish();
}

/// Registers strict JSON encoding benchmarks for every document size.
///
/// # Parameters
///
/// * `criterion` - Criterion context used to register the benchmark group.
///
/// # Panics
///
/// Panics when a generated document cannot be decoded into the encoding fixture
/// or a benchmark path cannot encode that fixture.
fn encode(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("budgeted-json-encode");

    for target_bytes in DOCUMENT_SIZES {
        let document = benchmark_document(target_bytes);
        let size = document.len();
        let fixture = serde_json::from_slice::<Fixture>(&document).expect("benchmark document must decode");
        let raw_value =
            RawValue::from_string(String::from_utf8(document.clone()).expect("benchmark document must be valid UTF-8"))
                .expect("benchmark document must be valid raw JSON");
        let numeric_fixture = vec![1.234_567_890_123_f64; target_bytes / 18];
        group.throughput(Throughput::Bytes(document.len() as u64));

        group.bench_with_input(
            BenchmarkId::new("encode/serde_json", size),
            &fixture,
            |bencher, fixture| {
                bencher.iter(|| black_box(serde_json::to_vec(black_box(fixture)).expect("fixture must encode")));
            },
        );
        let reused_session = JsonEncodeSession::from_limits(JsonEncodeLimits::<JsonResource, usize>::builder().build());
        let mut reused_encoder = JsonEncoder::new(reused_session);
        group.bench_with_input(
            BenchmarkId::new("encode/strict-only", size),
            &fixture,
            |bencher, fixture| {
                bencher.iter(|| black_box(reused_encoder.to_vec(black_box(fixture)).expect("fixture must encode")));
            },
        );
        group.bench_with_input(
            BenchmarkId::new("encode/value-only", size),
            &fixture,
            |bencher, fixture| {
                bencher.iter(|| {
                    let limits = JsonEncodeLimits::<JsonResource, usize>::builder()
                        .max_nodes(usize::MAX)
                        .build();
                    black_box(
                        JsonEncoder::with_limits(limits)
                            .to_vec(black_box(fixture))
                            .expect("fixture must fit the output budget"),
                    )
                });
            },
        );
        group.bench_with_input(
            BenchmarkId::new("encode/output-only", size),
            &fixture,
            |bencher, fixture| {
                bencher.iter(|| {
                    let limits = JsonEncodeLimits::<JsonResource, usize>::builder()
                        .max_output_bytes(target_bytes.saturating_add(RECORD.len()))
                        .build();
                    black_box(
                        JsonEncoder::with_limits(limits)
                            .to_vec(black_box(fixture))
                            .expect("fixture must fit the output budget"),
                    )
                });
            },
        );
        group.bench_with_input(BenchmarkId::new("encode/full", size), &fixture, |bencher, fixture| {
            bencher.iter(|| {
                let limits = JsonEncodeLimits::<JsonResource, usize>::builder()
                    .max_output_bytes(target_bytes.saturating_add(RECORD.len()))
                    .max_nodes(usize::MAX)
                    .build();
                black_box(
                    JsonEncoder::with_limits(limits)
                        .to_vec(black_box(fixture))
                        .expect("fixture must fit the complete budget"),
                )
            });
        });
        group.bench_with_input(
            BenchmarkId::new("encode/incremental-serde-json", size),
            &fixture,
            |bencher, fixture| {
                bencher.iter(|| {
                    let _: () =
                        serde_json::to_writer(std::io::sink(), black_box(fixture)).expect("fixture must encode");
                    black_box(())
                });
            },
        );
        group.bench_with_input(
            BenchmarkId::new("encode/incremental-writer", size),
            &fixture,
            |bencher, fixture| {
                bencher.iter(|| {
                    let session =
                        JsonEncodeSession::from_limits(JsonEncodeLimits::<JsonResource, usize>::builder().build());
                    black_box(JsonEncoder::new(session).write_incremental(std::io::sink(), black_box(fixture)))
                });
            },
        );
        group.bench_with_input(
            BenchmarkId::new("encode/incremental-output-only", size),
            &fixture,
            |bencher, fixture| {
                bencher.iter(|| {
                    let limits = JsonEncodeLimits::<JsonResource, usize>::builder()
                        .max_output_bytes(target_bytes.saturating_add(RECORD.len()))
                        .build();
                    let session = JsonEncodeSession::from_limits(limits);
                    black_box(JsonEncoder::new(session).write_incremental(std::io::sink(), black_box(fixture)))
                });
            },
        );
        let numeric_size = serde_json::to_vec(&numeric_fixture)
            .expect("numeric fixture must encode")
            .len();
        group.throughput(Throughput::Bytes(numeric_size as u64));
        group.bench_with_input(
            BenchmarkId::new("encode/numeric-serde-json", numeric_size),
            &numeric_fixture,
            |bencher, fixture| {
                bencher
                    .iter(|| black_box(serde_json::to_vec(black_box(fixture)).expect("numeric fixture must encode")));
            },
        );
        group.bench_with_input(
            BenchmarkId::new("encode/numeric", numeric_size),
            &numeric_fixture,
            |bencher, fixture| {
                bencher.iter(|| {
                    black_box(
                        JsonEncoder::unlimited()
                            .to_vec(black_box(fixture))
                            .expect("numeric fixture must encode"),
                    )
                });
            },
        );
        group.throughput(Throughput::Bytes(document.len() as u64));
        group.bench_with_input(
            BenchmarkId::new("encode/raw-value-serde-json", size),
            &raw_value,
            |bencher, raw| {
                bencher.iter(|| black_box(serde_json::to_vec(black_box(raw)).expect("raw fixture must encode")));
            },
        );
        group.bench_with_input(
            BenchmarkId::new("encode/raw-value", size),
            &raw_value,
            |bencher, raw| {
                bencher.iter(|| {
                    black_box(
                        JsonEncoder::unlimited()
                            .to_vec(black_box(raw))
                            .expect("raw fixture must encode"),
                    )
                });
            },
        );
    }
    group.finish();
}

criterion_group!(benches, decode, encode);
criterion_main!(benches);
