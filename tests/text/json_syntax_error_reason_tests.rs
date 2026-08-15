// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests stable strict JSON syntax reason formatting.

use qubit_json::text::JsonSyntaxErrorReason;

/// Verifies syntax reasons retain their privacy-safe display text.
#[test]
fn test_json_syntax_error_reason_formats_unexpected_byte() {
    assert_eq!(
        JsonSyntaxErrorReason::UnexpectedByte { byte: b'x' }.to_string(),
        "unexpected byte 0x78",
    );
}
