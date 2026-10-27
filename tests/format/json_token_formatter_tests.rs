// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use qubit_json::format::JsonPrettyErrorKind;
use qubit_json::format::JsonPrettyOptions;
use qubit_json::format::format_json_tokens;
use qubit_json::format::write_json_tokens;

#[test]
fn test_output_limit_keeps_later_syntax_error_and_zero_output() {
    let options = JsonPrettyOptions::builder().build().expect("valid options");
    let mut output = Vec::new();
    let error = write_json_tokens(&mut output, b"[0,0,0,0,]", &options, Some(1))
        .expect_err("invalid tail must be diagnosed after the output limit is crossed");
    assert_eq!(error.kind(), JsonPrettyErrorKind::Syntax);
    assert!(output.is_empty(), "failed planning must not write any prefix");
}

#[test]
fn test_output_limit_accepts_exact_formatted_length() {
    let options = JsonPrettyOptions::builder().build().expect("valid options");
    let expected = format_json_tokens(b"[1,2]", &options, None).expect("valid JSON must format");
    assert_eq!(
        format_json_tokens(b"[1,2]", &options, Some(expected.len())).expect("exact limit must fit"),
        expected
    );
    let error = format_json_tokens(b"[1,2]", &options, Some(expected.len() - 1))
        .expect_err("one byte below the formatted length must fail");
    assert_eq!(error.kind(), JsonPrettyErrorKind::OutputLimit);
}

#[test]
fn test_output_limit_rejects_valid_document_without_writing() {
    let options = JsonPrettyOptions::builder().build().expect("valid options");
    let mut output = Vec::new();
    let error = write_json_tokens(&mut output, b"[0,0,0]", &options, Some(1))
        .expect_err("valid output above the maximum must fail");
    assert_eq!(error.kind(), JsonPrettyErrorKind::OutputLimit);
    assert!(output.is_empty(), "output limit failure must write no prefix");
}

#[test]
fn test_zero_output_limit_writes_nothing() {
    let options = JsonPrettyOptions::builder().build().expect("valid options");
    let mut output = Vec::new();
    let error = write_json_tokens(&mut output, b"0", &options, Some(0))
        .expect_err("zero output limit must reject a non-empty document");
    assert_eq!(error.kind(), JsonPrettyErrorKind::OutputLimit);
    assert!(output.is_empty(), "zero limit failure must not write bytes");
}

#[test]
fn test_indentation_size_overflow_is_a_capacity_error() {
    let options = JsonPrettyOptions::builder()
        .indent_width(usize::MAX)
        .build()
        .expect("valid options");
    let error = format_json_tokens(b"[[0]]", &options, None).expect_err("indent multiplication must be checked");
    assert_eq!(error.kind(), JsonPrettyErrorKind::Capacity);
}

#[test]
fn test_token_format_preserves_source_tokens_and_requested_whitespace() {
    let options = JsonPrettyOptions::builder()
        .indent_width(2)
        .max_indent_levels(Some(1))
        .trailing_newline(true)
        .build()
        .expect("valid options");
    let output =
        format_json_tokens(br#"{"n":1.00e+02,"s":"\\u0061"}"#, &options, None).expect("valid JSON must format");
    assert_eq!(output, b"{\n  \"n\": 1.00e+02,\n  \"s\": \"\\\\u0061\"\n}\n");
}
// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
