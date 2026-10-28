// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Requested-allocation diagnostics for JSON value comparison and Debug.

#[path = "unique_decode_support/allocation_counter.rs"]
mod allocation_counter;

use std::fmt;

use qubit_json::value::JsonValueRef;
use qubit_json::value::SafeJsonValue;
use serde_json::Map;
use serde_json::Value;

const WIDTHS: [usize; 3] = [32, 512, 8_192];
const DEPTHS: [usize; 3] = [8, 64, 256];

struct Sink {
    bytes: usize,
    limit: Option<usize>,
}

impl fmt::Write for Sink {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        if self.limit.is_some_and(|limit| self.bytes + text.len() > limit) {
            return Err(fmt::Error);
        }
        self.bytes += text.len();
        Ok(())
    }
}

fn tree(shape: &str, width: usize, changed: Option<usize>) -> Value {
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
        _ => unreachable!("known benchmark shape"),
    }
}

fn deep_tree(depth: usize) -> Value {
    let mut value = Value::Null;
    for _ in 0..depth {
        value = Value::Array(vec![value]);
    }
    value
}

fn report<F>(shape: &str, width: usize, depth: usize, representation: &str, operation: &str, outcome: &str, mut run: F)
where
    F: FnMut() -> bool,
{
    let actual = run();
    let expected = outcome == "success";
    assert_eq!(actual, expected, "benchmark case {shape}/{operation}/{outcome}");
    let _ = std::hint::black_box(actual);
    let (actual, measurement) = allocation_counter::measure(run);
    assert_eq!(
        actual, expected,
        "measured benchmark case {shape}/{operation}/{outcome}"
    );
    println!(
        "VALUE_READ_ALLOC,{shape},{width},{depth},{representation},{operation},{outcome},{},{},{}",
        measurement.allocation_count, measurement.allocated_request_bytes, measurement.reallocation_count
    );
}

fn main() {
    println!(
        "VALUE_READ_ALLOC,shape,width,depth,representation,operation,outcome,allocation_count,allocated_request_bytes,reallocation_count"
    );
    for shape in ["array", "object"] {
        for width in WIDTHS {
            let left_value = tree(shape, width, None);
            let equal_value = tree(shape, width, None);
            let first_difference = tree(shape, width, Some(0));
            let last_difference = tree(shape, width, Some(width - 1));
            let left = SafeJsonValue::new(left_value);
            let equal = SafeJsonValue::new(equal_value);
            let first = SafeJsonValue::new(first_difference);
            let last = SafeJsonValue::new(last_difference);
            let left_view = JsonValueRef::new(left.as_value());
            let equal_view = JsonValueRef::new(equal.as_value());
            let first_view = JsonValueRef::new(first.as_value());
            let last_view = JsonValueRef::new(last.as_value());

            for (representation, compare) in [("owner", 0_u8), ("view", 1_u8)] {
                let (left_value, equal_value, first_value, last_value) = if compare == 0 {
                    (&left, &equal, &first, &last)
                } else {
                    // The view wrapper keeps the same references alive for each measured call.
                    // The branch is outside the measured operation.
                    let equal_result = left_view == equal_view;
                    let first_result = left_view == first_view;
                    let last_result = left_view == last_view;
                    assert!(equal_result);
                    assert!(!first_result);
                    assert!(!last_result);
                    report(shape, width, 0, representation, "equal", "success", || {
                        left_view == equal_view
                    });
                    report(shape, width, 0, representation, "first_difference", "failure", || {
                        left_view == first_view
                    });
                    report(shape, width, 0, representation, "last_difference", "failure", || {
                        left_view == last_view
                    });
                    let mut sink = Sink { bytes: 0, limit: None };
                    let debug = fmt::write(&mut sink, format_args!("{left_view:?}")).is_ok();
                    assert!(debug);
                    report(shape, width, 0, representation, "debug", "success", || {
                        let mut sink = Sink { bytes: 0, limit: None };
                        fmt::write(&mut sink, format_args!("{left_view:?}")).is_ok()
                    });
                    report(shape, width, 0, representation, "debug", "failure", || {
                        let mut sink = Sink {
                            bytes: 0,
                            limit: Some(64),
                        };
                        fmt::write(&mut sink, format_args!("{left_view:?}")).is_ok()
                    });
                    continue;
                };
                report(shape, width, 0, representation, "equal", "success", || {
                    left_value == equal_value
                });
                report(shape, width, 0, representation, "first_difference", "failure", || {
                    left_value == first_value
                });
                report(shape, width, 0, representation, "last_difference", "failure", || {
                    left_value == last_value
                });
                report(shape, width, 0, representation, "debug", "success", || {
                    let mut sink = Sink { bytes: 0, limit: None };
                    fmt::write(&mut sink, format_args!("{left_value:?}")).is_ok()
                });
                report(shape, width, 0, representation, "debug", "failure", || {
                    let mut sink = Sink {
                        bytes: 0,
                        limit: Some(64),
                    };
                    fmt::write(&mut sink, format_args!("{left_value:?}")).is_ok()
                });
            }
        }
    }

    for depth in DEPTHS {
        let left = SafeJsonValue::new(deep_tree(depth));
        let equal = SafeJsonValue::new(deep_tree(depth));
        let left_view = JsonValueRef::new(left.as_value());
        let equal_view = JsonValueRef::new(equal.as_value());
        for (representation, is_view) in [("owner", false), ("view", true)] {
            if is_view {
                report("deep", 0, depth, representation, "equal", "success", || {
                    left_view == equal_view
                });
                report("deep", 0, depth, representation, "debug", "success", || {
                    let mut sink = Sink { bytes: 0, limit: None };
                    fmt::write(&mut sink, format_args!("{left_view:?}")).is_ok()
                });
            } else {
                report("deep", 0, depth, representation, "equal", "success", || left == equal);
                report("deep", 0, depth, representation, "debug", "success", || {
                    let mut sink = Sink { bytes: 0, limit: None };
                    fmt::write(&mut sink, format_args!("{left:?}")).is_ok()
                });
            }
        }
    }
    println!("Note: allocated_request_bytes is total requested allocation volume, not peak live memory.");
}
