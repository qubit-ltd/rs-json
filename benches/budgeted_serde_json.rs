// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Criterion benchmarks for budget-aware JSON decoding and encoding.

use std::fmt;
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
use qubit_json::format::JsonPrettyOptions;
use qubit_json::format::format_json_tokens;
use serde::Deserialize;
use serde::Serialize;
use serde::Serializer;
use serde::ser::SerializeSeq;
use serde::ser::SerializeStruct;
use serde_json::value::RawValue;

mod internal;

use crate::internal::BenchmarkRecord;

const DOCUMENT_SIZES: [usize; 3] = [1_024, 65_536, 1_048_576];
const RECORD: &[u8] = br#"{"id":7,"text":"benchmark-record"}"#;
const DOCUMENT_PREFIX: &[u8] = br#"{"records":["#;
const DOCUMENT_SUFFIX: &[u8] = br#"]}"#;
const SERDE_JSON_NUMBER_TOKEN: &str = concat!("$", "serde_json", ":", ":private::Number");
const NUMBER_LEXEME: &str = "1234567890.125";
const T10_PAYLOAD_SIZES: [usize; 2] = [1_024, 65_536];
const TOKEN_FORMAT_ITEMS: [usize; 3] = [32, 512, 4_096];

/// Benchmarks full token formatting with and without a small output limit.
fn benchmark_token_format_limit(criterion: &mut Criterion) {
    let options = JsonPrettyOptions::builder().build().expect("valid default options");
    let mut group = criterion.benchmark_group("token_format_limit");
    for count in TOKEN_FORMAT_ITEMS {
        let input = format!("[{}]", vec!["0"; count].join(",")).into_bytes();
        let output = format_json_tokens(&input, &options, None).expect("fixture must format");
        assert!(output.len() > 8, "fixture must exceed the rejection limit");
        let error = format_json_tokens(&input, &options, Some(8)).expect_err("fixture must exceed the rejection limit");
        assert_eq!(error.kind(), qubit_json::format::JsonPrettyErrorKind::OutputLimit);
        for (label, maximum) in [("unlimited-success", None), ("limit-8-reject", Some(8))] {
            group.bench_with_input(BenchmarkId::new(label, count), &input, |bencher, input| {
                bencher.iter(|| black_box(format_json_tokens(black_box(input), &options, maximum)));
            });
        }
    }
    group.finish();
}

/// Emits one valid private serde_json Number protocol value.
struct PrivateNumber(&'static str);

impl Serialize for PrivateNumber {
    /// Sends the numeric lexeme through serde_json's private Number contract.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct(SERDE_JSON_NUMBER_TOKEN, 1)?;
        state.serialize_field(SERDE_JSON_NUMBER_TOKEN, self.0)?;
        state.end()
    }
}

/// Holds a fixed number of identical private Number protocol values.
struct PrivateNumberFixture(Vec<PrivateNumber>);

impl Serialize for PrivateNumberFixture {
    /// Serializes the fixture as a JSON array of private Number values.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut sequence = serializer.serialize_seq(Some(self.0.len()))?;
        for number in &self.0 {
            sequence.serialize_element(number)?;
        }
        sequence.end()
    }
}

/// Formats a fixed ASCII payload in bounded chunks for `collect_str`.
struct DisplayPayload<'a>(&'a str);

impl fmt::Display for DisplayPayload<'_> {
    /// Writes the payload as 64-byte ASCII chunks.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for start in (0..self.0.len()).step_by(64) {
            let end = (start + 64).min(self.0.len());
            formatter.write_str(&self.0[start..end])?;
        }
        Ok(())
    }
}

/// Routes a Display value through Serde's collected-string path.
struct CollectedDisplay<'a>(&'a str);

impl Serialize for CollectedDisplay<'_> {
    /// Delegates formatting to `Serializer::collect_str`.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_str(&DisplayPayload(self.0))
    }
}

/// Builds a repeatable ASCII payload with the requested byte length.
fn display_payload(size: usize) -> String {
    const CHUNK: &str = "display-collector-payload-0123456789";
    CHUNK.chars().cycle().take(size).collect()
}

/// Registers the T10 private Number and Display collector workloads.
fn benchmark_t10_workloads(criterion: &mut Criterion) {
    let mut number_group = criterion.benchmark_group("t10-private-number-protocol");
    for target_bytes in T10_PAYLOAD_SIZES {
        let count = target_bytes / NUMBER_LEXEME.len();
        let fixture = PrivateNumberFixture((0..count).map(|_| PrivateNumber(NUMBER_LEXEME)).collect());
        let payload_bytes = count * NUMBER_LEXEME.len();
        let output_bytes = count * NUMBER_LEXEME.len() + count.saturating_sub(1) + 2;
        number_group.throughput(Throughput::Bytes(output_bytes as u64));
        for (mode, bounded) in [("unlimited", false), ("exact-bounds", true)] {
            number_group.bench_with_input(
                BenchmarkId::new(format!("{mode}/number-count-{count}"), output_bytes),
                &fixture,
                |bencher, fixture| {
                    bencher.iter(|| {
                        let mut encoder = if bounded {
                            let limits = JsonEncodeLimits::<JsonResource, usize>::builder()
                                .max_output_bytes(output_bytes)
                                .max_nodes(count + 1)
                                .max_payload_bytes(payload_bytes)
                                .max_number_bytes(payload_bytes)
                                .build();
                            JsonEncoder::new(JsonEncodeSession::from_limits(limits))
                        } else {
                            JsonEncoder::unlimited()
                        };
                        black_box(
                            encoder
                                .to_vec(black_box(fixture))
                                .expect("private Number benchmark fixture must encode"),
                        );
                    });
                },
            );
        }
    }
    number_group.finish();

    let mut display_group = criterion.benchmark_group("t10-display-collector");
    for payload_bytes in T10_PAYLOAD_SIZES {
        let payload = display_payload(payload_bytes);
        let output_bytes = payload_bytes + 2;
        display_group.throughput(Throughput::Bytes(output_bytes as u64));
        for (mode, bounded) in [("unlimited", false), ("exact-bounds", true)] {
            display_group.bench_with_input(BenchmarkId::new(mode, output_bytes), &payload, |bencher, payload| {
                bencher.iter(|| {
                    let mut encoder = if bounded {
                        let limits = JsonEncodeLimits::<JsonResource, usize>::builder()
                            .max_output_bytes(output_bytes)
                            .max_nodes(1)
                            .max_payload_bytes(payload_bytes)
                            .build();
                        JsonEncoder::new(JsonEncodeSession::from_limits(limits))
                    } else {
                        JsonEncoder::unlimited()
                    };
                    black_box(
                        encoder
                            .to_vec(&CollectedDisplay(black_box(payload)))
                            .expect("Display collector benchmark fixture must encode"),
                    );
                });
            });
        }
    }
    display_group.finish();
}

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
    loop {
        let has_record = document.len() > DOCUMENT_PREFIX.len();
        let comma_len = usize::from(has_record);
        if document.len() + comma_len + RECORD.len() + DOCUMENT_SUFFIX.len() > target_bytes {
            break;
        }
        if has_record {
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
        JsonDecodeLimits::<JsonResource, usize>::default(),
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
                    let session =
                        JsonDecodeSession::from_limits(JsonDecodeLimits::<JsonResource, usize>::builder().build());
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
        let reused_session = JsonDecodeSession::from_limits(JsonDecodeLimits::<JsonResource, usize>::builder().build());
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
                    let limits = JsonDecodeLimits::<JsonResource, usize>::builder()
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
                            JsonDecodeSession::from_limits(JsonDecodeLimits::<JsonResource, usize>::builder().build()),
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
        let string_fixture = vec!["benchmark-string-payload"; target_bytes / 27];
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
            BenchmarkId::new("encode/strict-owned", size),
            &fixture,
            |bencher, fixture| {
                bencher.iter(|| {
                    black_box(
                        JsonEncoder::unlimited()
                            .to_vec(black_box(fixture))
                            .expect("fixture must encode"),
                    )
                });
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
                    JsonEncoder::new(session)
                        .write_incremental(&mut std::io::sink(), black_box(fixture))
                        .expect("the benchmark fixture must fit the configured budget");
                    black_box(())
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
                    JsonEncoder::new(session)
                        .write_incremental(&mut std::io::sink(), black_box(fixture))
                        .expect("the benchmark fixture must fit the configured budget");
                    black_box(())
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
        let string_size = serde_json::to_vec(&string_fixture)
            .expect("string fixture must encode")
            .len();
        group.throughput(Throughput::Bytes(string_size as u64));
        group.bench_with_input(
            BenchmarkId::new("encode/string-serde-json", string_size),
            &string_fixture,
            |bencher, fixture| {
                bencher.iter(|| black_box(serde_json::to_vec(black_box(fixture)).expect("string fixture must encode")));
            },
        );
        group.bench_with_input(
            BenchmarkId::new("encode/string", string_size),
            &string_fixture,
            |bencher, fixture| {
                bencher.iter(|| {
                    black_box(
                        JsonEncoder::unlimited()
                            .to_vec(black_box(fixture))
                            .expect("string fixture must encode"),
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

criterion_group!(
    benches,
    decode,
    encode,
    benchmark_t10_workloads,
    benchmark_token_format_limit
);
criterion_main!(benches);
