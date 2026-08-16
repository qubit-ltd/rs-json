// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests structured JSON syntax error reasons.

use qubit_json::decode::JsonSyntaxErrorReason;

#[test]
fn syntax_reasons_have_stable_display_text() {
    assert_eq!(
        JsonSyntaxErrorReason::ExpectedColon.to_string(),
        "expected ':'"
    );
    assert_eq!(
        JsonSyntaxErrorReason::UnexpectedByte { byte: b'x' }.to_string(),
        "unexpected byte 0x78",
    );
}
