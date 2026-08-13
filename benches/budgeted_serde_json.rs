// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Criterion benchmarks for budget-aware JSON decoding and encoding.

use std::hint::black_box;

use criterion::Criterion;
use criterion::criterion_group;
use criterion::criterion_main;
use qubit_budget::json::JsonDecodeLimits;
use qubit_budget::json::JsonDecodeSession;
use qubit_budget::json::JsonEncodeLimits;
use qubit_budget::json::JsonEncodeSession;
use qubit_json::text::decode_slice;
use qubit_json::text::encode_to_vec;
use serde::Deserialize;
use serde::Serialize;

const DOCUMENT: &[u8] = br#"{"name":"qubit","items":[1,2,3,4],"enabled":true}"#;

#[derive(Debug, Deserialize, Serialize)]
struct Fixture {
    name: String,
    items: Vec<u64>,
    enabled: bool,
}

fn decode(c: &mut Criterion) {
    c.bench_function("json decode with budget", |b| {
        b.iter(|| {
            let mut session = JsonDecodeSession::owned(JsonDecodeLimits::empty());
            black_box(decode_slice::<Fixture, _, _>(DOCUMENT, &mut session).unwrap())
        });
    });
}

fn decode_baseline(c: &mut Criterion) {
    c.bench_function("json decode with serde_json", |b| {
        b.iter(|| {
            black_box(
                serde_json::from_slice::<Fixture>(DOCUMENT)
                    .expect("benchmark document should decode"),
            )
        });
    });
}

fn encode(c: &mut Criterion) {
    let fixture = Fixture {
        name: String::from("qubit"),
        items: vec![1, 2, 3, 4],
        enabled: true,
    };
    c.bench_function("json encode with budget", |b| {
        b.iter(|| {
            let mut session = JsonEncodeSession::owned(JsonEncodeLimits::empty());
            black_box(encode_to_vec(&fixture, &mut session).unwrap())
        });
    });
}

fn encode_baseline(c: &mut Criterion) {
    let fixture = Fixture {
        name: String::from("qubit"),
        items: vec![1, 2, 3, 4],
        enabled: true,
    };
    c.bench_function("json encode with serde_json", |b| {
        b.iter(|| {
            black_box(serde_json::to_vec(&fixture).expect("benchmark fixture should encode"))
        });
    });
}

criterion_group!(benches, decode, decode_baseline, encode, encode_baseline);
criterion_main!(benches);
