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
use qubit_json::decode::NormalizingJsonDecodeOptions;
use qubit_json::decode::NormalizingJsonDecoder;
use qubit_json::encode::JsonEncoder;
use serde::Deserialize;
use serde::Serialize;

mod internal;

use internal::BenchmarkRecord;

const DOCUMENT_SIZES: [usize; 3] = [1_024, 65_536, 1_048_576];
const RECORD: &[u8] = br#"{"id":7,"text":"benchmark-record"}"#;
const DOCUMENT_PREFIX: &[u8] = br#"{"records":["#;
const DOCUMENT_SUFFIX: &[u8] = br#"]}"#;

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
        target_bytes
            >= DOCUMENT_PREFIX.len() + RECORD.len() + DOCUMENT_SUFFIX.len(),
        "benchmark target must hold one record",
    );

    let mut document = Vec::with_capacity(target_bytes);
    document.extend_from_slice(DOCUMENT_PREFIX);
    while document.len() + RECORD.len() + DOCUMENT_SUFFIX.len() <= target_bytes
    {
        if document.len() > DOCUMENT_PREFIX.len() {
            document.push(b',');
        }
        document.extend_from_slice(RECORD);
    }
    document.extend_from_slice(DOCUMENT_SUFFIX);

    assert!(
        document.len() <= target_bytes
            && target_bytes - document.len() <= RECORD.len(),
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
    let decoder =
        NormalizingJsonDecoder::new(NormalizingJsonDecodeOptions::strict());
    let mut group = criterion.benchmark_group("budgeted-json-decode");

    for target_bytes in DOCUMENT_SIZES {
        let document = benchmark_document(target_bytes);
        let size = document.len();
        let input = std::str::from_utf8(&document)
            .expect("benchmark document must be valid UTF-8");
        let fixture = serde_json::from_slice::<Fixture>(&document)
            .expect("benchmark document must decode");
        black_box(fixture.records.len());
        group.throughput(Throughput::Bytes(document.len() as u64));

        group.bench_with_input(
            BenchmarkId::new("decode/serde_json", size),
            &document,
            |bencher, input| {
                bencher.iter(|| {
                    black_box(
                        serde_json::from_slice::<Fixture>(black_box(input))
                            .expect("fixture must decode"),
                    )
                });
            },
        );
        group.bench_with_input(
            BenchmarkId::new("decode/owned-session", size),
            &document,
            |bencher, input| {
                bencher.iter(|| {
                    let mut session = JsonDecodeSession::owned(
                        JsonDecodeLimits::<JsonResource, usize>::builder()
                            .build(),
                    );
                    black_box(
                        JsonDecoder::new(&mut session)
                            .decode::<Fixture>(black_box(input))
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
                    let mut value = JsonValueBudget::new(
                        JsonValueLimits::<JsonResource, usize>::builder()
                            .build(),
                    );
                    let mut session =
                        JsonDecodeSession::borrowing_value(&mut value);
                    black_box(
                        JsonDecoder::new(&mut session)
                            .decode::<Fixture>(black_box(input))
                            .expect("fixture must decode"),
                    )
                });
            },
        );
        let mut reused_session = JsonDecodeSession::owned(
            JsonDecodeLimits::<JsonResource, usize>::builder().build(),
        );
        group.bench_with_input(
            BenchmarkId::new("decode/reused-session", size),
            &document,
            |bencher, input| {
                bencher.iter(|| {
                    black_box(
                        JsonDecoder::new(&mut reused_session)
                            .decode::<Fixture>(black_box(input))
                            .expect("fixture must decode"),
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
                            .decode::<Fixture>(black_box(input))
                            .expect("fixture must decode"),
                    )
                });
            },
        );
        let mut lenient_session = JsonDecodeSession::owned(
            JsonDecodeLimits::<JsonResource, usize>::builder().build(),
        );
        group.bench_with_input(
            BenchmarkId::new("decode/lenient-json-decoder-with-session", size),
            &input,
            |bencher, input| {
                bencher.iter(|| {
                    black_box(
                        decoder
                            .decode_with_session::<Fixture>(
                                black_box(input),
                                &mut lenient_session,
                            )
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
        let fixture = serde_json::from_slice::<Fixture>(&document)
            .expect("benchmark document must decode");
        group.throughput(Throughput::Bytes(document.len() as u64));

        group.bench_with_input(
            BenchmarkId::new("encode/serde_json", size),
            &fixture,
            |bencher, fixture| {
                bencher.iter(|| {
                    black_box(
                        serde_json::to_vec(black_box(fixture))
                            .expect("fixture must encode"),
                    )
                });
            },
        );
        group.bench_with_input(
            BenchmarkId::new("encode/owned-session", size),
            &fixture,
            |bencher, fixture| {
                bencher.iter(|| {
                    let mut session = JsonEncodeSession::owned(
                        JsonEncodeLimits::<JsonResource, usize>::builder()
                            .build(),
                    );
                    black_box(
                        JsonEncoder::new(&mut session)
                            .to_vec(black_box(fixture))
                            .expect("fixture must encode"),
                    )
                });
            },
        );
        group.bench_with_input(
            BenchmarkId::new("encode/borrowed-session", size),
            &fixture,
            |bencher, fixture| {
                bencher.iter(|| {
                    let mut value = JsonValueBudget::new(
                        JsonValueLimits::<JsonResource, usize>::builder()
                            .build(),
                    );
                    let mut session =
                        JsonEncodeSession::borrowing_value(&mut value);
                    black_box(
                        JsonEncoder::new(&mut session)
                            .to_vec(black_box(fixture))
                            .expect("fixture must encode"),
                    )
                });
            },
        );
        let mut reused_session = JsonEncodeSession::owned(
            JsonEncodeLimits::<JsonResource, usize>::builder().build(),
        );
        group.bench_with_input(
            BenchmarkId::new("encode/reused-session", size),
            &fixture,
            |bencher, fixture| {
                bencher.iter(|| {
                    black_box(
                        JsonEncoder::new(&mut reused_session)
                            .to_vec(black_box(fixture))
                            .expect("fixture must encode"),
                    )
                });
            },
        );
        group.bench_with_input(
            BenchmarkId::new("encode/incremental-writer", size),
            &fixture,
            |bencher, fixture| {
                bencher.iter(|| {
                    let mut session = JsonEncodeSession::owned(
                        JsonEncodeLimits::<JsonResource, usize>::builder()
                            .build(),
                    );
                    black_box(
                        JsonEncoder::new(&mut session).write_incremental(
                            std::io::sink(),
                            black_box(fixture),
                        ),
                    )
                });
            },
        );
    }
    group.finish();
}

criterion_group!(benches, decode, encode);
criterion_main!(benches);
