// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Criterion comparisons for bounded JCS rendering and the previous
//! canonicalizer.

#[cfg(feature = "jcs-allocation-diagnostics")]
use std::alloc::GlobalAlloc;
#[cfg(feature = "jcs-allocation-diagnostics")]
use std::alloc::Layout;
#[cfg(feature = "jcs-allocation-diagnostics")]
use std::alloc::System;
use std::hint::black_box;
#[cfg(feature = "jcs-allocation-diagnostics")]
use std::sync::atomic::AtomicBool;
#[cfg(feature = "jcs-allocation-diagnostics")]
use std::sync::atomic::AtomicUsize;
#[cfg(feature = "jcs-allocation-diagnostics")]
use std::sync::atomic::Ordering;

use criterion::BenchmarkId;
use criterion::Criterion;
use criterion::Throughput;
use criterion::criterion_group;
use criterion::criterion_main;
use qubit_budget::json::JsonEncodeLimits;
use qubit_budget::json::JsonResource;
use qubit_json::encode::JcsEncoder;
use serde_json::Map;
use serde_json::Value;

const LARGE_OBJECT_ENTRIES: usize = 2_048;
const DEEP_OBJECT_DEPTH: usize = 128;
const LARGE_ARRAY_ITEMS: usize = 16_384;
const FINITE_WORKING_BYTES: usize = 1024 * 1024;

#[cfg(feature = "jcs-allocation-diagnostics")]
struct AllocationCounter;

#[cfg(feature = "jcs-allocation-diagnostics")]
static ALLOCATION_MEASUREMENT_ACTIVE: AtomicBool = AtomicBool::new(false);
#[cfg(feature = "jcs-allocation-diagnostics")]
static ALLOCATION_COUNT: AtomicUsize = AtomicUsize::new(0);
#[cfg(feature = "jcs-allocation-diagnostics")]
static ALLOCATED_BYTES: AtomicUsize = AtomicUsize::new(0);

#[cfg(feature = "jcs-allocation-diagnostics")]
#[global_allocator]
static GLOBAL_ALLOCATOR: AllocationCounter = AllocationCounter;

#[cfg(feature = "jcs-allocation-diagnostics")]
unsafe impl GlobalAlloc for AllocationCounter {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc(layout) };
        record_allocation(pointer, layout.size());
        pointer
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc_zeroed(layout) };
        record_allocation(pointer, layout.size());
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) };
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let resized = unsafe { System.realloc(pointer, layout, new_size) };
        record_allocation(resized, new_size);
        resized
    }
}

#[cfg(feature = "jcs-allocation-diagnostics")]
fn record_allocation(pointer: *mut u8, size: usize) {
    if !pointer.is_null() && ALLOCATION_MEASUREMENT_ACTIVE.load(Ordering::Relaxed) {
        ALLOCATION_COUNT.fetch_add(1, Ordering::Relaxed);
        ALLOCATED_BYTES.fetch_add(size, Ordering::Relaxed);
    }
}

/// Counts allocations made by one operation, excluding report formatting and
/// Criterion samples.
///
/// # Parameters
///
/// * `shape` — the fixture shape label.
/// * `path` — the encoding path label.
/// * `operation` — the operation whose allocations are measured.
///
/// # Returns
///
/// This function returns after printing the allocation report.
#[cfg(feature = "jcs-allocation-diagnostics")]
fn report_allocations(shape: &str, path: &str, operation: impl FnOnce() -> Vec<u8>) {
    ALLOCATION_COUNT.store(0, Ordering::Relaxed);
    ALLOCATED_BYTES.store(0, Ordering::Relaxed);
    ALLOCATION_MEASUREMENT_ACTIVE.store(true, Ordering::SeqCst);
    let output = operation();
    ALLOCATION_MEASUREMENT_ACTIVE.store(false, Ordering::SeqCst);

    eprintln!(
        "JCS_ALLOC shape={shape} path={path} allocation_count={} allocated_bytes={} output_bytes={} (includes output buffers and all operation allocations)",
        ALLOCATION_COUNT.load(Ordering::Relaxed),
        ALLOCATED_BYTES.load(Ordering::Relaxed),
        output.len(),
    );
    drop(output);
}

/// Builds an object with many members to exercise sorting-table capacity.
///
/// # Returns
///
/// The large object fixture.
fn large_object() -> Value {
    let mut entries = Map::with_capacity(LARGE_OBJECT_ENTRIES);
    for index in 0..LARGE_OBJECT_ENTRIES {
        entries.insert(
            format!("record-{index:05}"),
            serde_json::json!({"id": index, "payload": "benchmark-value"}),
        );
    }
    Value::Object(entries)
}

/// Builds a nested object without recursive construction.
///
/// # Returns
///
/// The deeply nested object fixture.
fn deep_object() -> Value {
    let mut value = serde_json::json!({"leaf": true});
    for depth in (0..DEEP_OBJECT_DEPTH).rev() {
        value = serde_json::json!({"depth": depth, "child": value});
    }
    value
}

/// Builds a wide scalar array.
///
/// # Returns
///
/// The large scalar array fixture.
fn large_array() -> Value {
    Value::Array((0..LARGE_ARRAY_ITEMS).map(Value::from).collect())
}

/// Counts the nodes used to configure a generous finite value budget.
///
/// # Parameters
///
/// * `root` — the value whose nodes are counted.
///
/// # Returns
///
/// The number of value nodes reachable from `root`.
fn node_count(root: &Value) -> usize {
    let mut count = 0;
    let mut pending = vec![root];
    while let Some(value) = pending.pop() {
        count += 1;
        match value {
            Value::Array(values) => pending.extend(values.iter()),
            Value::Object(entries) => pending.extend(entries.values()),
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
        }
    }
    count
}

/// Registers comparisons using a fresh encoder session for each sample.
///
/// # Parameters
///
/// * `criterion` — the Criterion harness that receives the benchmark group.
///
/// # Returns
///
/// This function returns after registering and finishing the benchmark group.
fn benchmark_jcs_encoding(criterion: &mut Criterion) {
    let fixtures = [
        ("large-object", large_object()),
        ("deep-object", deep_object()),
        ("large-array", large_array()),
    ];
    #[cfg(feature = "jcs-allocation-diagnostics")]
    if std::env::var_os("JCS_ALLOCATION_DIAGNOSTICS").is_some() {
        for (shape, value) in &fixtures {
            let finite_limits = JsonEncodeLimits::<JsonResource, usize>::builder()
                .max_output_bytes(
                    serde_json_canonicalizer::to_vec(value)
                        .expect("benchmark fixture must be accepted by the legacy canonicalizer")
                        .len(),
                )
                .max_nodes(node_count(value))
                .build();
            report_allocations(shape, "jcs-unlimited", || {
                let mut encoder = JcsEncoder::unlimited();
                encoder.to_vec(value).expect("unlimited JCS encoding must succeed")
            });
            report_allocations(shape, "jcs-finite-budgets", || {
                let mut encoder = JcsEncoder::with_limits(finite_limits, FINITE_WORKING_BYTES);
                encoder.to_vec(value).expect("fixture must fit finite budgets")
            });
            report_allocations(shape, "legacy-canonicalizer", || {
                serde_json_canonicalizer::to_vec(value).expect("legacy canonicalizer must encode the fixture")
            });
        }
        return;
    }

    let mut group = criterion.benchmark_group("jcs-encode");

    for (shape, value) in fixtures {
        let canonical = serde_json_canonicalizer::to_vec(&value)
            .expect("benchmark fixture must be accepted by the legacy canonicalizer");
        let size = canonical.len();
        let finite_limits = JsonEncodeLimits::<JsonResource, usize>::builder()
            .max_output_bytes(size)
            .max_nodes(node_count(&value))
            .build();
        group.throughput(Throughput::Bytes(size as u64));

        group.bench_with_input(
            BenchmarkId::new(format!("{shape}/unlimited"), size),
            &value,
            |bencher, value| {
                bencher.iter(|| {
                    let mut encoder = JcsEncoder::unlimited();
                    black_box(
                        encoder
                            .to_vec(black_box(value))
                            .expect("unlimited JCS encoding must succeed"),
                    )
                });
            },
        );
        group.bench_with_input(
            BenchmarkId::new(format!("{shape}/finite-budgets"), size),
            &value,
            |bencher, value| {
                bencher.iter(|| {
                    let mut encoder = JcsEncoder::with_limits(finite_limits, FINITE_WORKING_BYTES);
                    black_box(
                        encoder
                            .to_vec(black_box(value))
                            .expect("fixture must fit finite budgets"),
                    )
                });
            },
        );
        group.bench_with_input(
            BenchmarkId::new(format!("{shape}/legacy-canonicalizer"), size),
            &value,
            |bencher, value| {
                bencher.iter(|| {
                    black_box(
                        serde_json_canonicalizer::to_vec(black_box(value))
                            .expect("legacy canonicalizer must encode the fixture"),
                    )
                });
            },
        );
    }
    group.finish();
}

criterion_group!(benches, benchmark_jcs_encoding);
criterion_main!(benches);
