// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use qubit_budget::json::JsonResource;

use crate::lexical::JsonLexicalError;
use crate::lexical::JsonLexicalErrorReason;
use crate::lexical::JsonLexicalFailure;
use crate::lexical::JsonLexicalScanError;

#[test]
fn test_lexical_scan_error_formats_each_variant() {
    let lexical: JsonLexicalError<JsonResource, usize> = JsonLexicalError::Syntax(JsonLexicalFailure {
        offset: 2,
        line: 1,
        column: 3,
        reason: JsonLexicalErrorReason::UnexpectedByte,
    });
    let converted: JsonLexicalScanError<&str, JsonResource, usize> = lexical.into();
    assert!(converted.to_string().contains("UnexpectedByte"));

    let consumer: JsonLexicalScanError<&str, JsonResource, usize> = JsonLexicalScanError::Consumer("stopped");
    assert_eq!(consumer.to_string(), "stopped");

    let limit: JsonLexicalScanError<&str, JsonResource, usize> = JsonLexicalScanError::EncodedStringLimit {
        maximum: 4,
        observed: 5,
        offset: 9,
    };
    assert_eq!(limit.to_string(), "encoded JSON string limit exceeded");
}
