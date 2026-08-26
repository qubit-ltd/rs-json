// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests the independent JSON number-contract classifier.

use qubit_json_fuzz::json_number_contract::numbers_fit_contract;

/// Verifies every inclusive numeric boundary is admitted.
#[test]
fn test_numbers_fit_contract_accepts_supported_boundaries() {
    let input = br#"{
        "signed": -9223372036854775808,
        "unsigned": 18446744073709551615,
        "fraction": 1.7976931348623157e308,
        "numeric text": "18446744073709551616"
    }"#;

    serde_json::from_slice::<serde_json::Value>(input).expect("the reference input must be valid JSON");
    assert!(numbers_fit_contract(input));
}

/// Verifies integers and floats outside the supported ranges are rejected.
#[test]
fn test_numbers_fit_contract_rejects_unsupported_ranges() {
    for input in [&b"-9223372036854775809"[..], &b"18446744073709551616"[..]] {
        serde_json::from_slice::<serde_json::Value>(input).expect("serde_json must accept the reference input");
        assert!(
            !numbers_fit_contract(input),
            "number must be outside the contract: {input:?}"
        );
    }
    assert!(!numbers_fit_contract(b"1e400"));
}
