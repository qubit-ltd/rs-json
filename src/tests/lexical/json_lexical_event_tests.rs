// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests the synchronous event contract of the shared lexical scanner.

use qubit_budget::json::JsonDecodeLimits;
use qubit_budget::json::JsonDecodeSession;
use qubit_budget::json::JsonResource;

use crate::lexical::JsonLexicalError;
use crate::lexical::JsonLexicalEvent;
use crate::lexical::JsonLexicalScanner;
use crate::lexical::line_column_at;

const INPUT: &[u8] = br#"{"x":[null,true,-0,"\u0078"],"empty":{}}"#;

fn scanner() -> JsonDecodeSession<'static, JsonResource, usize> {
    let limits = JsonDecodeLimits::<JsonResource, usize>::builder().build();
    JsonDecodeSession::from_limits(limits)
}

fn scan_outcome(
    input: &[u8],
    limits: JsonDecodeLimits<JsonResource, usize>,
    with_events: bool,
) -> (String, Option<usize>, Option<usize>) {
    let mut session = JsonDecodeSession::from_limits(limits);
    let mut attempt = session.begin_value();
    let transaction = attempt.value_transaction_mut();
    let error = {
        let mut scanner = JsonLexicalScanner::new(transaction, true);
        let result = if with_events {
            scanner.scan_with_events(input, |_| {})
        } else {
            scanner.scan(input)
        };
        format!("{result:?}")
    };
    (error, transaction.used_nodes(), transaction.used_payload_bytes())
}

#[test]
fn test_json_lexical_events_preserve_token_sequence_and_offsets() {
    let mut session = scanner();
    let mut attempt = session.begin_value();
    let mut events = Vec::new();
    let result = JsonLexicalScanner::new(attempt.value_transaction_mut(), false)
        .scan_with_events(INPUT, |event| events.push(event));

    assert!(result.is_ok());
    let observed = events
        .iter()
        .map(|event| match event {
            JsonLexicalEvent::BeginObject { offset } => ("begin-object", *offset, None),
            JsonLexicalEvent::Key { token, offset } => ("key", *offset, Some(*token)),
            JsonLexicalEvent::BeginArray { offset } => ("begin-array", *offset, None),
            JsonLexicalEvent::Null { offset } => ("null", *offset, None),
            JsonLexicalEvent::Boolean { value, offset } => (if *value { "true" } else { "false" }, *offset, None),
            JsonLexicalEvent::Number { token, offset } => ("number", *offset, Some(*token)),
            JsonLexicalEvent::String { token, offset } => ("string", *offset, Some(*token)),
            JsonLexicalEvent::EndArray { offset } => ("end-array", *offset, None),
            JsonLexicalEvent::EndObject { offset } => ("end-object", *offset, None),
        })
        .collect::<Vec<_>>();

    assert_eq!(
        observed.iter().map(|entry| entry.0).collect::<Vec<_>>(),
        [
            "begin-object",
            "key",
            "begin-array",
            "null",
            "true",
            "number",
            "string",
            "end-array",
            "key",
            "begin-object",
            "end-object",
            "end-object"
        ],
    );
    assert_eq!(
        observed.iter().map(|entry| entry.1).collect::<Vec<_>>(),
        [0, 1, 5, 6, 11, 16, 19, 27, 29, 37, 38, 39],
    );
    assert_eq!(observed[1].1, 1);
    assert_eq!(observed[1].2, Some(&br#""x""#[..]));
    assert_eq!(observed[5].1, 16);
    assert_eq!(observed[5].2, Some(&b"-0"[..]));
    assert_eq!(observed[6].1, 19);
    assert_eq!(observed[6].2, Some(&br#""\u0078""#[..]));
    assert_eq!(observed[8].1, 29);
    assert_eq!(observed[8].2, Some(&br#""empty""#[..]));
}

#[test]
fn test_json_lexical_scan_and_event_scan_share_syntax_errors() {
    for input in [&b"[true] trailing"[..], &br#"["\uD800"]"#[..], &b"01"[..]] {
        let mut plain_session = scanner();
        let mut plain_attempt = plain_session.begin_value();
        let plain = JsonLexicalScanner::new(plain_attempt.value_transaction_mut(), false).scan(input);

        let mut event_session = scanner();
        let mut event_attempt = event_session.begin_value();
        let event =
            JsonLexicalScanner::new(event_attempt.value_transaction_mut(), false).scan_with_events(input, |_| {});

        match (plain, event) {
            (Err(JsonLexicalError::Syntax(left)), Err(JsonLexicalError::Syntax(right))) => {
                assert_eq!(left, right, "scanner paths diverged for {input:?}");
            }
            _ => panic!("both scanner paths must reject {input:?}"),
        }
    }
}

#[test]
fn test_line_column_at_uses_utf8_columns_and_crlf_lines() {
    let input = "{\r\n  \"é\": ?}".as_bytes();
    let offset = input
        .iter()
        .position(|byte| *byte == b'?')
        .expect("test input has marker");

    assert_eq!(line_column_at(input, offset), (2, 8));
}

#[test]
fn test_json_lexical_scan_paths_share_budget_failures_and_staged_usage() {
    let cases = [
        (
            b"123".as_slice(),
            JsonDecodeLimits::<JsonResource, usize>::builder()
                .max_number_bytes(2)
                .build(),
            "number byte limit",
            "NumberBytes",
        ),
        (
            b"[null]".as_slice(),
            JsonDecodeLimits::<JsonResource, usize>::builder().max_nodes(1).build(),
            "node limit",
            "Nodes",
        ),
        (
            b"[null]".as_slice(),
            JsonDecodeLimits::<JsonResource, usize>::builder().max_depth(1).build(),
            "depth limit",
            "Depth",
        ),
    ];

    for (input, limits, description, resource) in cases {
        let plain = scan_outcome(input, limits, false);
        let with_events = scan_outcome(input, limits, true);

        assert_eq!(plain, with_events, "scanner paths diverged at {description}");
        assert!(
            plain.0.contains("Budget"),
            "expected a budget error at {description}: {}",
            plain.0
        );
        assert!(
            plain.0.contains(resource),
            "expected the {resource} failure at {description}: {}",
            plain.0
        );
    }
}
