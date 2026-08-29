// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests compound budget-aware JSON serialization.

use qubit_budget::json::JsonEncodeLimits;
use qubit_budget::json::JsonEncodeSession;
use qubit_budget::json::JsonResource;
use serde::Serialize;
use serde::Serializer;

use crate::text::json_encode_test_support::encode;

#[derive(Serialize)]
struct Pair {
    left: u8,
    right: u8,
}

#[derive(Serialize)]
struct Triple(u8, u8);

struct Bytes;

impl Serialize for Bytes {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_bytes(&[1, 10, 100])
    }
}

/// Verifies compound serialization preserves object members.
#[test]
fn test_json_encode_compound_serializes_struct_members() {
    let mut session = JsonEncodeSession::from_limits(JsonEncodeLimits::<JsonResource, usize>::builder().build());
    let output = encode(&Pair { left: 1, right: 2 }, &mut session).expect("compound JSON should serialize");

    assert_eq!(output, br#"{"left":1,"right":2}"#);
}

/// Verifies tuple-struct compounds complete through the decorated end path.
#[test]
fn test_json_encode_compound_serializes_tuple_struct_fields() {
    let mut session = JsonEncodeSession::from_limits(JsonEncodeLimits::<JsonResource, usize>::builder().build());
    let output = encode(&Triple(3, 4), &mut session).expect("tuple-struct JSON should serialize");

    assert_eq!(output, b"[3,4]");
}

/// Verifies scalar serializer paths instantiate with byte-sized quantities.
#[test]
fn test_json_encode_scalar_paths_support_u8_quantities() {
    macro_rules! assert_scalar {
        ($value:expr) => {{
            let mut session = JsonEncodeSession::<JsonResource, u8>::from_limits(
                JsonEncodeLimits::<JsonResource, u8>::builder().build(),
            );
            encode(&$value, &mut session).expect("scalar should fit an unconfigured budget");
        }};
    }

    assert_scalar!(-1_i8);
    assert_scalar!(-1_i16);
    assert_scalar!(-1_i32);
    assert_scalar!(-1_i64);
    assert_scalar!(-1_i128);
    assert_scalar!(1_u8);
    assert_scalar!(1_u16);
    assert_scalar!(1_u32);
    assert_scalar!(1_u64);
    assert_scalar!(1_u128);
    assert_scalar!(1.5_f32);
    assert_scalar!(2.5_f64);
    assert_scalar!('x');
    assert_scalar!("text");
    assert_scalar!(Some(1_u8));
    assert_scalar!(None::<u8>);
    assert_scalar!(());
    assert_scalar!(vec![1_u8]);
    assert_scalar!(Bytes);
}
