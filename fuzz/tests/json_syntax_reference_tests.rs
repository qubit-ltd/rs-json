// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Capability tests for the independent JSON syntax reference.

use qubit_json_fuzz::json_syntax_reference::validate_json_syntax;

#[test]
fn test_syntax_reference_accepts_non_materialized_numbers_and_literal_keys() {
    for input in [
        "1e400",
        "18446744073709551616",
        r#"{"x":1,"x":2}"#,
        r#"{"$serde_json::private::RawValue":"null"}"#,
    ] {
        assert!(validate_json_syntax(input.as_bytes()).is_ok(), "{input}");
    }
}

#[test]
fn test_syntax_reference_rejects_malformed_complete_documents() {
    for input in ["[", "[1,]", "{} {}", "1 2", "01", "1e+", r#""\q""#] {
        assert!(validate_json_syntax(input.as_bytes()).is_err(), "{input}");
    }
    assert!(validate_json_syntax(&[0xff]).is_err());
}
