// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Requested-allocation diagnostics for protected JSON projection and bridging.

#[path = "unique_decode_support/allocation_counter.rs"]
mod allocation_counter;

use std::process::Command;

use qubit_budget::json::JsonResource;
use qubit_budget::json::JsonValueBudget;
use qubit_budget::json::JsonValueLimits;
use qubit_json::value::JsonValueEncoder;
use qubit_json::value::SafeJsonValue;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Map;
use serde_json::Value;

const WIDTHS: [usize; 3] = [32, 512, 8_192];
const DEPTHS: [usize; 3] = [8, 64, 256];
/// A byte slice serialized through Serde's distinct bytes event.
struct Bytes<'a>(&'a [u8]);

impl Serialize for Bytes<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_bytes(self.0)
    }
}

/// Runs a read-only Git command and returns its trimmed UTF-8 output.
fn git_output(args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .output()
        .expect("git must be available when running the benchmark");
    assert!(output.status.success(), "git command must succeed: {args:?}");
    String::from_utf8(output.stdout)
        .expect("git output must be UTF-8")
        .trim()
        .to_owned()
}

/// Detects which serde_json object-map configuration this binary uses.
fn map_feature() -> &'static str {
    let mut map = Map::new();
    map.insert("z".to_owned(), Value::Null);
    map.insert("a".to_owned(), Value::Null);
    if map.keys().next().is_some_and(|key| key == "z") {
        "preserve_order"
    } else {
        "default"
    }
}

/// Builds an array chain with the requested number of containers.
fn chain(depth: usize) -> Value {
    let mut value = Value::Null;
    for _ in 0..depth {
        value = Value::Array(vec![value]);
    }
    value
}

/// Builds unique short-key or shared-prefix long-key object data.
fn object_fixture(width: usize, long_key: bool) -> Value {
    let prefix = "p".repeat(128);
    let entries = (0..width).map(|index| {
        let key = if long_key {
            format!("{prefix}-{index:05}")
        } else {
            format!("k{index:05}")
        };
        (key, Value::from(index as u64))
    });
    Value::Object(entries.collect())
}

/// Measures direct SafeJsonValue deserialization, then validates outside the
/// window.
fn bridge(input: &str, expected: &Value, shape: &str, size: usize, feature: &str, revision: &str) {
    let (owner, measurement) = allocation_counter::measure(|| {
        if shape == "chain" && size > 128 {
            let mut deserializer = serde_json::Deserializer::from_str(input);
            deserializer.disable_recursion_limit();
            let owner = SafeJsonValue::deserialize(&mut deserializer).expect("deep bridge fixture must deserialize");
            deserializer.end().expect("deep bridge fixture must be fully consumed");
            owner
        } else {
            serde_json::from_str::<SafeJsonValue>(input).expect("bridge fixture must deserialize")
        }
    });
    assert_eq!(owner.as_value(), expected, "{shape}/{size} bridge changed the value");
    emit(shape, size, "bridge", feature, revision, measurement);
    std::hint::black_box(owner);
}

/// Measures transaction-aware projection, then validates and commits outside
/// the window.
fn project<T: Serialize + ?Sized>(
    source: &T,
    expected: &Value,
    shape: &str,
    size: usize,
    feature: &str,
    revision: &str,
) {
    let mut budget = JsonValueBudget::new(JsonValueLimits::<JsonResource, usize>::new());
    let mut transaction = budget.transaction();
    let (owner, measurement) = allocation_counter::measure(|| {
        JsonValueEncoder::new()
            .encode_in_transaction(source, &mut transaction)
            .expect("projection fixture must encode")
    });
    assert_eq!(
        owner.as_value(),
        expected,
        "{shape}/{size} projection changed the value"
    );
    transaction.commit().expect("projection transaction must commit");
    emit(shape, size, "project", feature, revision, measurement);
    std::hint::black_box(owner);
}

/// Prints one CSV row after the operation and its validation have completed.
fn emit(
    shape: &str,
    size: usize,
    operation: &str,
    feature: &str,
    revision: &str,
    measurement: allocation_counter::AllocationMeasurement,
) {
    println!(
        "VALUE_PROJECTION_ALLOC,{shape},{size},{operation},{feature},{revision},{},{},{},{},{},{},{}",
        measurement.allocation_count,
        measurement.allocated_request_bytes,
        measurement.reallocation_count,
        measurement.baseline_live_bytes,
        measurement.peak_live_bytes,
        measurement.end_live_bytes,
        measurement.peak_extra_bytes,
    );
}

fn main() {
    allocation_counter::self_check();
    let feature = map_feature();
    let revision = git_output(&["rev-parse", "HEAD"]);
    let dirty_files = git_output(&["status", "--porcelain=v1"]);
    let source_hashes = Command::new("sha256sum")
        .args([
            "Cargo.toml",
            "benches/value_projection_allocations.rs",
            "src/value/internal/safe_value_builder.rs",
            "src/value/json_value_encoder/internal/json_value_serializer.rs",
            "src/value/json_value_encoder/internal/json_value_compound.rs",
            "src/value/internal/value_visitor.rs",
        ])
        .output()
        .expect("sha256sum must be available when running the benchmark");
    assert!(source_hashes.status.success(), "source checksum must succeed");
    println!("VALUE_PROJECTION_META,feature,{feature}");
    println!("VALUE_PROJECTION_META,head,{revision}");
    println!("VALUE_PROJECTION_META,rustc,{}", rustc_version());
    let lock_hash = Command::new("sha256sum")
        .arg("Cargo.lock")
        .output()
        .expect("sha256sum must be available when running the benchmark");
    assert!(lock_hash.status.success(), "Cargo.lock checksum must succeed");
    let lock_hash = String::from_utf8(lock_hash.stdout).expect("sha256sum output must be UTF-8");
    println!(
        "VALUE_PROJECTION_META,cargo_lock_sha256,{}",
        lock_hash.split_whitespace().next().unwrap_or("unknown")
    );
    println!("VALUE_PROJECTION_META,dirty_files,{}", dirty_files.replace('\n', " | "));
    println!(
        "VALUE_PROJECTION_META,source_sha256,{}",
        String::from_utf8(source_hashes.stdout)
            .expect("sha256sum output must be UTF-8")
            .replace('\n', " | ")
    );
    println!(
        "VALUE_PROJECTION_ALLOC,shape,size,operation,feature,revision,allocation_count,allocated_request_bytes,reallocation_count,baseline_live_bytes,peak_live_bytes,end_live_bytes,peak_extra_bytes"
    );

    for width in WIDTHS {
        let sequence = Value::Array((0..width).map(|index| Value::from(index as u64)).collect());
        project(&sequence, &sequence, "sequence", width, feature, &revision);
        let bytes = (0..width).map(|index| (index % 256) as u8).collect::<Vec<_>>();
        let bytes_expected = Value::Array(bytes.iter().map(|byte| Value::from(*byte)).collect());
        project(&Bytes(&bytes), &bytes_expected, "bytes", width, feature, &revision);

        let short_object = object_fixture(width, false);
        project(&short_object, &short_object, "object_short", width, feature, &revision);
        let short_text = serde_json::to_string(&short_object).expect("short object must serialize");
        bridge(&short_text, &short_object, "object_short", width, feature, &revision);

        let long_object = object_fixture(width, true);
        project(
            &long_object,
            &long_object,
            "object_long_prefix",
            width,
            feature,
            &revision,
        );
        let long_text = serde_json::to_string(&long_object).expect("long object must serialize");
        bridge(
            &long_text,
            &long_object,
            "object_long_prefix",
            width,
            feature,
            &revision,
        );
    }

    for depth in DEPTHS {
        let value = chain(depth);
        project(&value, &value, "chain", depth, feature, &revision);
        let input = serde_json::to_string(&value).expect("chain fixture must serialize");
        bridge(&input, &value, "chain", depth, feature, &revision);
    }
}

/// Returns the active Rust compiler version for reproducibility metadata.
fn rustc_version() -> String {
    let output = Command::new("rustc")
        .arg("--version")
        .output()
        .expect("rustc must be available when running the benchmark");
    assert!(output.status.success(), "rustc --version must succeed");
    String::from_utf8(output.stdout)
        .expect("rustc output must be UTF-8")
        .trim()
        .to_owned()
}
