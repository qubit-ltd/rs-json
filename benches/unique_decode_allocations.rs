// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Requested allocation diagnostics for unique-key decoding paths.

#[path = "unique_decode_support/allocation_counter.rs"]
mod allocation_counter;
mod unique_decode_support;

use qubit_json::decode::JsonDecoder;
use unique_decode_support::decode_path::DecodePath;
use unique_decode_support::decode_path::assert_decoded;
use unique_decode_support::decode_path::assert_duplicate_rejected;
use unique_decode_support::decode_path::decode_batch_with_decoder;
use unique_decode_support::decode_path::decode_once;
use unique_decode_support::decode_path::decode_reject_once;
use unique_decode_support::decode_path::decode_with_decoder;
use unique_decode_support::decode_path::finite_decoder;
use unique_decode_support::fixtures::Shape;
use unique_decode_support::fixtures::fixture;
use unique_decode_support::fixtures::reject_fixture;

const SIZES: [usize; 3] = [1024, 64 * 1024, 1024 * 1024];

fn main() {
    println!(
        "UNIQUE_DECODE_ALLOC,shape,bytes,path,budget,mode,allocation_count,allocated_request_bytes,reallocation_count"
    );
    for shape in Shape::ALL {
        for requested in SIZES {
            let fixture = fixture(shape, requested);
            for path in DecodePath::ALL {
                for finite in [false, true] {
                    if path.is_direct_serde() && finite {
                        continue;
                    }
                    let budget = if path.is_direct_serde() {
                        "no_budget"
                    } else if finite {
                        "finite"
                    } else {
                        "unlimited"
                    };
                    let mut decoder = if finite {
                        finite_decoder(&fixture, 1)
                    } else {
                        JsonDecoder::unlimited()
                    };
                    let warmup = decode_with_decoder(path, &fixture, &mut decoder);
                    assert_decoded(&warmup, path, shape);
                    std::hint::black_box(warmup);
                    let (fresh_result, fresh) = allocation_counter::measure(|| decode_once(path, &fixture, finite));
                    assert_decoded(&fresh_result, path, shape);
                    std::hint::black_box(fresh_result);
                    println!(
                        "UNIQUE_DECODE_ALLOC,{},{},{},{},fresh,{},{},{}",
                        shape.name(),
                        fixture.bytes(),
                        path.name(),
                        budget,
                        fresh.allocation_count,
                        fresh.allocated_request_bytes,
                        fresh.reallocation_count
                    );

                    let mut warmup_decoder = if finite {
                        finite_decoder(&fixture, 8)
                    } else {
                        JsonDecoder::unlimited()
                    };
                    let warmup = decode_batch_with_decoder(path, &fixture, &mut warmup_decoder, 8);
                    for decoded in warmup {
                        assert_decoded(&decoded, path, shape);
                        std::hint::black_box(decoded);
                    }
                    let mut decoder = if finite {
                        finite_decoder(&fixture, 8)
                    } else {
                        JsonDecoder::unlimited()
                    };
                    let (batch_result, batch) =
                        allocation_counter::measure(|| decode_batch_with_decoder(path, &fixture, &mut decoder, 8));
                    for decoded in batch_result {
                        assert_decoded(&decoded, path, shape);
                        std::hint::black_box(decoded);
                    }
                    println!(
                        "UNIQUE_DECODE_ALLOC,{},{},{},{},batch8,{},{},{}",
                        shape.name(),
                        fixture.bytes(),
                        path.name(),
                        budget,
                        batch.allocation_count,
                        batch.allocated_request_bytes,
                        batch.reallocation_count
                    );
                }
            }
            let reject = reject_fixture(shape, requested);
            for finite in [false, true] {
                let budget = if finite { "finite" } else { "unlimited" };
                let warmup = assert_duplicate_rejected(decode_reject_once(&reject, finite));
                let _ = std::hint::black_box(warmup);
                let (rejection_result, rejected) = allocation_counter::measure(|| decode_reject_once(&reject, finite));
                let rejection_error = assert_duplicate_rejected(rejection_result);
                let _ = std::hint::black_box(rejection_error);
                println!(
                    "UNIQUE_DECODE_ALLOC,{},{},unique_reject,{},fresh,{},{},{}",
                    shape.name(),
                    reject.bytes(),
                    budget,
                    rejected.allocation_count,
                    rejected.allocated_request_bytes,
                    rejected.reallocation_count
                );
            }
        }
    }
}
