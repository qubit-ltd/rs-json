// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests for the two-byte fuzz limit mapping.

use super::fuzz_limit::limit;

/// Verifies the mapping reaches both inclusive endpoints.
#[test]
fn test_limit_maps_u16_values_to_inclusive_range() {
    assert_eq!(limit(&[0, 0], 0), 1);
    assert_eq!(limit(&[255, 15], 0), 4096);
}

/// Verifies missing bytes use a deterministic minimum limit.
#[test]
fn test_limit_defaults_missing_bytes_to_one() {
    assert_eq!(limit(&[], 0), 1);
    assert_eq!(limit(&[7], 0), 1 + 7);
    assert_eq!(limit(&[0, 0, 9], 2), 10);
}
