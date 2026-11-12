// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Criterion benchmarks for mutable and read-only JSON tree traversal.

use std::fmt;
use std::hint::black_box;

use criterion::BatchSize;
use criterion::BenchmarkId;
use criterion::Criterion;
use criterion::criterion_group;
use criterion::criterion_main;
use qubit_budget::json::JsonResource;
use qubit_budget::json::JsonValueBudget;
use qubit_budget::json::JsonValueLimits;
use qubit_json::value::JsonValueRef;
use qubit_json::value::SafeJsonValue;
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
const READ_WIDTHS: [usize; 3] = [32, 512, 8_192];
const READ_DEPTHS: [usize; 3] = [8, 64, 256];

struct DiscardSink;

impl fmt::Write for DiscardSink {
    fn write_str(&mut self, _text: &str) -> fmt::Result {
        Ok(())
    }
}

fn large_read_value(shape: &str, width: usize, changed: Option<usize>) -> Value {
    match shape {
        "array" => Value::Array(
            (0..width)
                .map(|index| Value::from(if changed == Some(index) { -1 } else { index as i64 }))
                .collect(),
        ),
        "object" => {
            let mut entries = Map::with_capacity(width);
            for index in 0..width {
                entries.insert(
                    format!("key-{index:05}"),
                    Value::from(if changed == Some(index) { -1 } else { index as i64 }),
                );
            }
            Value::Object(entries)
        }
        _ => unreachable!("known read benchmark shape"),
    }
}

fn nested_array(depth: usize) -> Value {
    let mut value = Value::Null;
    for _ in 0..depth {
        value = Value::Array(vec![value]);
    }
    value
}

fn debug_to_discard<T: fmt::Debug>(value: &T) -> fmt::Result {
    fmt::write(&mut DiscardSink, format_args!("{value:?}"))
}

/// Measures owner/view comparison and Debug without storing formatted output.
fn benchmark_value_reads(c: &mut Criterion) {
    let mut group = c.benchmark_group("json-value-read");
    macro_rules! compare_case {
        ($shape:expr, $representation:literal, $operation:literal, $width:expr, $left:expr, $right:expr) => {
            group.bench_function(
                format!("compare/{}/{}/{}/{}", $shape, $representation, $operation, $width),
                |bencher| bencher.iter(|| black_box($left) == black_box($right)),
            );
        };
    }
    for shape in ["array", "object"] {
        for width in READ_WIDTHS {
            let left = SafeJsonValue::new(large_read_value(shape, width, None));
            let equal = SafeJsonValue::new(large_read_value(shape, width, None));
            let first = SafeJsonValue::new(large_read_value(shape, width, Some(0)));
            let last = SafeJsonValue::new(large_read_value(shape, width, Some(width - 1)));
            let left_view = JsonValueRef::new(left.as_value());
            let equal_view = JsonValueRef::new(equal.as_value());
            let first_view = JsonValueRef::new(first.as_value());
            let last_view = JsonValueRef::new(last.as_value());

            compare_case!(shape, "owner", "equal", width, &left, &equal);
            compare_case!(shape, "owner", "first-difference", width, &left, &first);
            compare_case!(shape, "owner", "last-difference", width, &left, &last);
            compare_case!(shape, "view", "equal", width, left_view, equal_view);
            compare_case!(shape, "view", "first-difference", width, left_view, first_view);
            compare_case!(shape, "view", "last-difference", width, left_view, last_view);

            group.bench_function(format!("debug/{shape}/owner/{width}"), |bencher| {
                bencher.iter(|| debug_to_discard(black_box(&left)).expect("discard sink accepts Debug output"));
            });
            group.bench_function(format!("debug/{shape}/view/{width}"), |bencher| {
                bencher.iter(|| debug_to_discard(black_box(&left_view)).expect("discard sink accepts Debug output"));
            });
        }
    }

    for depth in READ_DEPTHS {
        let left = SafeJsonValue::new(nested_array(depth));
        let equal = SafeJsonValue::new(nested_array(depth));
        let left_view = JsonValueRef::new(left.as_value());
        let equal_view = JsonValueRef::new(equal.as_value());
        group.bench_function(format!("compare/deep/owner/equal/{depth}"), |bencher| {
            bencher.iter(|| black_box(&left) == black_box(&equal));
        });
        group.bench_function(format!("compare/deep/view/equal/{depth}"), |bencher| {
            bencher.iter(|| black_box(left_view) == black_box(equal_view));
        });
        group.bench_function(format!("debug/deep/owner/{depth}"), |bencher| {
            bencher.iter(|| debug_to_discard(black_box(&left)).expect("discard sink accepts Debug output"));
        });
        group.bench_function(format!("debug/deep/view/{depth}"), |bencher| {
            bencher.iter(|| debug_to_discard(black_box(&left_view)).expect("discard sink accepts Debug output"));
        });
    }
    group.finish();
}

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
                    Value::Object(entries) => {
                        pending.extend(entries.values().rev().map(Frame::Enter));
                    }
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

criterion_group!(benches, benchmark_read, benchmark_mut, benchmark_value_reads);
criterion_main!(benches);
