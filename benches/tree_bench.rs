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

/// Returns either an unlimited configuration or a generous active node limit.
fn traversal_limits(bounded: bool) -> JsonValueLimits<JsonResource, usize> {
    if bounded {
        JsonValueLimits::builder().max_nodes(usize::MAX).build()
    } else {
        JsonValueLimits::new()
    }
}

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

impl JsonTreeMutVisitor for RedactionShapeVisitor {
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

/// Measures the minimum explicit-stack work for enter/leave tree callbacks.
fn visitor_floor(root: &Value) -> usize {
    enum Frame<'value> {
        Enter(&'value Value),
        Leave(&'value Value),
    }

    let mut callbacks = 0_usize;
    let mut pending = vec![Frame::Enter(root)];
    while let Some(frame) = pending.pop() {
        match frame {
            Frame::Enter(value) => {
                callbacks += 1;
                black_box(value);
                pending.push(Frame::Leave(value));
                match value {
                    Value::Array(values) => pending.extend(values.iter().rev().map(Frame::Enter)),
                    Value::Object(entries) => pending.extend(entries.values().rev().map(Frame::Enter)),
                    Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
                }
            }
            Frame::Leave(value) => {
                callbacks += 1;
                black_box(value);
            }
        }
    }
    callbacks
}

/// Registers read-only traversal benchmarks for wide and deep trees.
fn benchmark_read(c: &mut Criterion) {
    let mut group = c.benchmark_group("json-tree-read");

    for size in ARRAY_SIZES {
        let value = large_array(size);
        group.bench_with_input(
            BenchmarkId::new("visitor-floor/large-array", size),
            &value,
            |bencher, value| {
                bencher.iter(|| black_box(visitor_floor(black_box(value))));
            },
        );
        for (mode, bounded) in [("unlimited", false), ("bounded", true)] {
            group.bench_with_input(
                BenchmarkId::new(format!("{mode}/large-array"), size),
                &value,
                |bencher, value| {
                    bencher.iter_batched(
                        || JsonValueBudget::new(traversal_limits(bounded)),
                        |mut budget| {
                            let mut transaction = budget.transaction();
                            let mut visitor = ReadVisitor;
                            JsonTreeReader::new(&mut transaction)
                                .process(black_box(value), &mut visitor)
                                .expect("read traversal succeeds");
                            transaction.commit().expect("tree transaction commits");
                            let _ = black_box(budget);
                        },
                        BatchSize::SmallInput,
                    );
                },
            );
        }
    }

    for size in OBJECT_SIZES {
        let value = large_object(size);
        group.bench_with_input(
            BenchmarkId::new("visitor-floor/large-object", size),
            &value,
            |bencher, value| {
                bencher.iter(|| black_box(visitor_floor(black_box(value))));
            },
        );
        for (mode, bounded) in [("unlimited", false), ("bounded", true)] {
            group.bench_with_input(
                BenchmarkId::new(format!("{mode}/large-object"), size),
                &value,
                |bencher, value| {
                    bencher.iter_batched(
                        || JsonValueBudget::new(traversal_limits(bounded)),
                        |mut budget| {
                            let mut transaction = budget.transaction();
                            let mut visitor = ReadVisitor;
                            JsonTreeReader::new(&mut transaction)
                                .process(black_box(value), &mut visitor)
                                .expect("read traversal succeeds");
                            transaction.commit().expect("tree transaction commits");
                            let _ = black_box(budget);
                        },
                        BatchSize::SmallInput,
                    );
                },
            );
        }
    }

    let value = deep_tree(DEEP_TREE_DEPTH);
    group.bench_function("visitor-floor/deep-tree", |bencher| {
        bencher.iter(|| black_box(visitor_floor(black_box(&value))));
    });
    for (mode, bounded) in [("unlimited", false), ("bounded", true)] {
        group.bench_function(format!("{mode}/deep-tree"), |bencher| {
            bencher.iter_batched(
                || JsonValueBudget::new(traversal_limits(bounded)),
                |mut budget| {
                    let mut transaction = budget.transaction();
                    let mut visitor = ReadVisitor;
                    JsonTreeReader::new(&mut transaction)
                        .process(black_box(&value), &mut visitor)
                        .expect("read traversal succeeds");
                    transaction.commit().expect("tree transaction commits");
                    let _ = black_box(budget);
                },
                BatchSize::SmallInput,
            );
        });
    }
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
        for (mode, input_bounded, output_bounded) in [
            ("unlimited", false, false),
            ("input-bounded", true, false),
            ("output-bounded", false, true),
            ("fully-bounded", true, true),
        ] {
            group.bench_with_input(
                BenchmarkId::new(format!("{mode}/{name}"), "redaction-shape"),
                &value,
                |bencher, value| {
                    bencher.iter_batched(
                        || value.clone(),
                        |mut value| {
                            let mut input_budget = JsonValueBudget::new(traversal_limits(input_bounded));
                            let mut output_budget = JsonValueBudget::new(traversal_limits(output_bounded));
                            let mut input = input_budget.transaction();
                            let mut output = output_budget.transaction();
                            let mut visitor = RedactionShapeVisitor;
                            JsonTreeMutator::new(&mut input, &mut output)
                                .process(black_box(&mut value), &mut visitor)
                                .expect("mutable traversal succeeds");
                            input.commit().expect("input transaction commits");
                            output.commit().expect("output transaction commits");
                            black_box(value);
                        },
                        BatchSize::SmallInput,
                    );
                },
            );
        }
    }
    group.finish();
}

criterion_group!(benches, benchmark_read, benchmark_mut);
criterion_main!(benches);
