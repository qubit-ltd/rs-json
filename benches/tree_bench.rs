// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Criterion benchmarks for mutable and read-only JSON tree traversal.

use std::hint::black_box;

use criterion::BatchSize;
use criterion::BenchmarkId;
use criterion::Criterion;
use criterion::criterion_group;
use criterion::criterion_main;
use qubit_budget::json::JsonResource;
use qubit_budget::json::JsonValueBudget;
use qubit_budget::json::JsonValueLimits;
use qubit_json::value::traverse::JsonTreeContext;
use qubit_json::value::traverse::JsonTreeControl;
use qubit_json::value::traverse::JsonTreeMutVisitor;
use qubit_json::value::traverse::JsonTreeMutator;
use qubit_json::value::traverse::JsonTreeReader;
use qubit_json::value::traverse::JsonTreeVisitor;
use serde_json::Map;
use serde_json::Value;

const ARRAY_SIZES: [usize; 2] = [1_024, 16_384];
const OBJECT_SIZES: [usize; 2] = [256, 4_096];
const DEEP_TREE_DEPTH: usize = 128;

/// Visits every node without changing the input tree.
struct ReadVisitor;

impl JsonTreeVisitor for ReadVisitor {
    type Error = std::convert::Infallible;

    /// Accepts one node before the processor visits its descendants.
    fn enter(&mut self, _value: &Value, _context: JsonTreeContext<'_>) -> Result<(), Self::Error> {
        Ok(())
    }

    /// Accepts one node after the processor visits its descendants.
    fn leave(&mut self, _value: &Value, _context: JsonTreeContext<'_>) -> Result<(), Self::Error> {
        Ok(())
    }
}

/// Mirrors the object-key inspection performed by redaction visitors.
struct RedactionShapeVisitor;

impl JsonTreeMutVisitor<qubit_budget::json::JsonResource, usize> for RedactionShapeVisitor {
    type Error = std::convert::Infallible;

    /// Removes the synthetic secret field and descends into the remaining
    /// tree.
    fn visit(&mut self, value: &mut Value, _context: JsonTreeContext<'_>) -> Result<JsonTreeControl, Self::Error> {
        if let Value::Object(entries) = value {
            entries.remove("secret");
        }
        Ok(JsonTreeControl::Descend)
    }
}

/// Builds a large scalar array.
fn large_array(size: usize) -> Value {
    Value::Array((0..size).map(|index| Value::from(index as u64)).collect())
}

/// Builds a large object whose entries resemble redaction input.
fn large_object(size: usize) -> Value {
    let mut entries = Map::with_capacity(size);
    for index in 0..size {
        entries.insert(
            format!("record-{index}"),
            serde_json::json!({"keep": index, "secret": "TOP_SECRET"}),
        );
    }
    Value::Object(entries)
}

/// Builds a deeply nested object without relying on recursive construction.
fn deep_tree(depth: usize) -> Value {
    let mut value = serde_json::json!({"secret": "TOP_SECRET", "leaf": true});
    for level in (0..depth).rev() {
        value = serde_json::json!({
            "level": level,
            "secret": "TOP_SECRET",
            "child": value,
        });
    }
    value
}

/// Registers read-only traversal benchmarks for wide and deep trees.
fn benchmark_read(c: &mut Criterion) {
    let mut group = c.benchmark_group("json-tree-read");

    for size in ARRAY_SIZES {
        let value = large_array(size);
        group.bench_with_input(BenchmarkId::new("large-array", size), &value, |bencher, value| {
            bencher.iter(|| {
                let mut budget = JsonValueBudget::new(JsonValueLimits::<JsonResource, usize>::builder().build());
                let mut transaction = budget.transaction();
                let mut visitor = ReadVisitor;
                JsonTreeReader::new(&mut transaction)
                    .process(black_box(value), &mut visitor)
                    .expect("unlimited read traversal succeeds");
                transaction.commit();
                let _ = black_box(budget);
            });
        });
    }

    for size in OBJECT_SIZES {
        let value = large_object(size);
        group.bench_with_input(BenchmarkId::new("large-object", size), &value, |bencher, value| {
            bencher.iter(|| {
                let mut budget = JsonValueBudget::new(JsonValueLimits::<JsonResource, usize>::builder().build());
                let mut transaction = budget.transaction();
                let mut visitor = ReadVisitor;
                JsonTreeReader::new(&mut transaction)
                    .process(black_box(value), &mut visitor)
                    .expect("unlimited read traversal succeeds");
                transaction.commit();
                let _ = black_box(budget);
            });
        });
    }

    let value = deep_tree(DEEP_TREE_DEPTH);
    group.bench_function("deep-tree", |bencher| {
        bencher.iter(|| {
            let mut budget = JsonValueBudget::new(JsonValueLimits::<JsonResource, usize>::builder().build());
            let mut transaction = budget.transaction();
            let mut visitor = ReadVisitor;
            JsonTreeReader::new(&mut transaction)
                .process(black_box(&value), &mut visitor)
                .expect("unlimited read traversal succeeds");
            transaction.commit();
            let _ = black_box(budget);
        });
    });
    group.finish();
}

/// Registers mutable traversal benchmarks using a redaction-shaped visitor.
fn benchmark_mut(c: &mut Criterion) {
    let mut group = c.benchmark_group("json-tree-mut");

    for (name, value) in [
        ("large-array", large_array(16_384)),
        ("large-object", large_object(4_096)),
        ("deep-tree", deep_tree(DEEP_TREE_DEPTH)),
    ] {
        group.bench_with_input(BenchmarkId::new(name, "redaction-shape"), &value, |bencher, value| {
            bencher.iter_batched(
                || value.clone(),
                |mut value| {
                    let mut budget = JsonValueBudget::new(JsonValueLimits::<JsonResource, usize>::builder().build());
                    let mut transaction = budget.transaction();
                    let mut visitor = RedactionShapeVisitor;
                    JsonTreeMutator::new(&mut transaction)
                        .process(black_box(&mut value), &mut visitor)
                        .expect("unlimited mutable traversal succeeds");
                    transaction.commit();
                    black_box(value);
                },
                BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

criterion_group!(benches, benchmark_read, benchmark_mut);
criterion_main!(benches);
