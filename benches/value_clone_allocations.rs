// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Requested-allocation diagnostics for SafeJsonValue::copy_from and Clone.

#[path = "unique_decode_support/allocation_counter.rs"]
mod allocation_counter;

use std::process::Command;

use qubit_json::value::SafeJsonValue;
use serde_json::Map;
use serde_json::Value;

const WIDTHS: [usize; 3] = [32, 512, 8_192];
const DEPTHS: [usize; 3] = [8, 64, 256];

fn git_revision() -> String {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .expect("git rev-parse must be available when running the benchmark");
    assert!(output.status.success(), "git rev-parse HEAD failed");
    String::from_utf8(output.stdout)
        .expect("git revision must be UTF-8")
        .trim()
        .to_owned()
}

fn map_feature() -> &'static str {
    let mut map = Map::new();
    map.insert("z".to_owned(), Value::Null);
    map.insert("a".to_owned(), Value::Null);
    let mut keys = map.keys();
    if keys.next().is_some_and(|key| key == "z") {
        "preserve_order"
    } else {
        "default"
    }
}

fn fixture(shape: &str, size: usize) -> SafeJsonValue {
    let value = match shape {
        "array" => Value::Array((0..size).map(|index| Value::from(index as u64)).collect()),
        "object_fixed_key" | "object_escaped_non_ascii_key" => {
            let mut map = Map::with_capacity(size);
            for index in 0..size {
                let key = if shape == "object_fixed_key" {
                    format!("key-{index:05}")
                } else {
                    format!("键-\\\"{index:05}\n")
                };
                map.insert(key, Value::from(index as u64));
            }
            Value::Object(map)
        }
        "mixed_deep_chain" => {
            let mut value = Value::Null;
            for level in 0..size {
                value = if level % 2 == 0 {
                    Value::Array(vec![value])
                } else {
                    let mut map = Map::new();
                    map.insert("层-\"\n".to_owned(), value);
                    Value::Object(map)
                };
            }
            value
        }
        _ => unreachable!("known fixture shape"),
    };
    SafeJsonValue::new(value)
}

fn report(source: &SafeJsonValue, shape: &str, size: usize, operation: &str, feature: &str, revision: &str) {
    let (copy, measurement) = allocation_counter::measure(|| match operation {
        "copy_from" => SafeJsonValue::copy_from(source.as_value()),
        "clone" => source.clone(),
        _ => unreachable!("known copy operation"),
    });
    assert_eq!(&copy, source, "{shape}/{size}/{operation} changed the value");
    std::hint::black_box(&copy);
    println!(
        "VALUE_CLONE_ALLOC,{shape},{size},{feature},{operation},{revision},{},{},{},{},{},{},{}",
        measurement.allocation_count,
        measurement.allocated_request_bytes,
        measurement.reallocation_count,
        measurement.baseline_live_bytes,
        measurement.peak_live_bytes,
        measurement.end_live_bytes,
        measurement.peak_extra_bytes,
    );
    drop(copy);
}

fn main() {
    allocation_counter::self_check();
    let feature = map_feature();
    let revision = git_revision();
    println!(
        "VALUE_CLONE_ALLOC,shape,size,feature,operation,revision,allocation_count,allocated_request_bytes,reallocation_count,baseline_live_bytes,peak_live_bytes,end_live_bytes,peak_extra_bytes"
    );
    for (shape, sizes) in [
        ("array", &WIDTHS[..]),
        ("object_fixed_key", &WIDTHS[..]),
        ("object_escaped_non_ascii_key", &WIDTHS[..]),
        ("mixed_deep_chain", &DEPTHS[..]),
    ] {
        for &size in sizes {
            let source = fixture(shape, size);
            report(&source, shape, size, "copy_from", feature, &revision);
            report(&source, shape, size, "clone", feature, &revision);
        }
    }
    println!(
        "Note: allocated_request_bytes is total successful allocation request volume, including realloc request sizes; peak_extra_bytes is peak live bytes above the pre-operation baseline. These are allocator request diagnostics, not RSS."
    );
}
