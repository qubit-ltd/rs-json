// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests lexical JSON admission resource accounting.

use qubit_budget::BudgetError;
use qubit_budget::Observation;
use qubit_budget::ResourceLimit;
use qubit_budget::StructureLimits;
use qubit_budget::json::JsonDecodeLimits;
use qubit_budget::json::JsonDecodeSession;
use qubit_budget::json::JsonResource;
use qubit_budget::json::JsonValueLimits;
use qubit_json::decode::JsonDecodeError;
use qubit_json::decode::JsonDecoder;
use serde::de::IgnoredAny;

/// Verifies object keys, strings, and numbers consume one shared payload
/// budget.
#[test]
fn test_json_lexical_preflight_consumes_payload_for_keys_strings_and_numbers() {
    let limits = JsonDecodeLimits::<JsonResource, usize>::builder()
        .value_limits(
            JsonValueLimits::<JsonResource, usize>::builder()
                .payload_bytes_limit(ResourceLimit::new(
                    JsonResource::PayloadBytes,
                    4,
                ))
                .build(),
        )
        .build();
    let mut session = JsonDecodeSession::owned(limits);
    let error = JsonDecoder::new(session)
        .decode_utf8::<IgnoredAny>(br#"{"a":"bc","n":12}"#)
        .expect_err(
            "one key, string, and number must exceed four payload bytes",
        );

    assert!(matches!(
        error,
        JsonDecodeError::Budget(error)
            if matches!(
                error.budget_error(),
                Some(BudgetError::Insufficient {
                    resource: JsonResource::PayloadBytes,
                    limit: 4,
                    remaining: 0,
                    requested: 2,
                })
            )
    ));
}

/// Verifies decoded escapes use their decoded UTF-8 byte length for key limits.
#[test]
fn test_json_lexical_preflight_charges_decoded_key_bytes() {
    let limits = JsonDecodeLimits::<JsonResource, usize>::builder()
        .value_limits(
            JsonValueLimits::<JsonResource, usize>::builder()
                .structure_limits(
                    StructureLimits::<JsonResource, usize>::builder()
                        .key_bytes_limit(ResourceLimit::new(
                            JsonResource::KeyBytes,
                            2,
                        )),
                )
                .build(),
        )
        .build();
    let mut session = JsonDecodeSession::owned(limits);
    let error = JsonDecoder::new(session)
        .decode_utf8::<IgnoredAny>(br#"{"\u4e2d":null}"#)
        .expect_err(
            "the decoded three-byte key must exceed the two-byte limit",
        );

    assert!(matches!(
        error,
        JsonDecodeError::Budget(error)
            if matches!(
                error.budget_error(),
                Some(BudgetError::LimitExceeded {
                    resource: JsonResource::KeyBytes,
                    observed: Observation::Exact(3),
                    maximum: 2,
                })
            )
    ));
}

/// Verifies each JSON value consumes exactly one node from the shared session.
#[test]
fn test_json_lexical_preflight_charges_each_value_node() {
    let limits =
        JsonDecodeLimits::<JsonResource, usize>::builder()
            .value_limits(
                JsonValueLimits::<JsonResource, usize>::builder()
                    .structure_limits(StructureLimits::builder().nodes_limit(
                        ResourceLimit::new(JsonResource::Nodes, 1),
                    ))
                    .build(),
            )
            .build();
    let mut session = JsonDecodeSession::owned(limits);
    let error = JsonDecoder::new(session)
        .decode_utf8::<IgnoredAny>(br#"{"value":true}"#)
        .expect_err("the object child must exceed the one-node budget");

    assert!(matches!(
        error,
        JsonDecodeError::Budget(error)
            if matches!(
                error.budget_error(),
                Some(BudgetError::Insufficient {
                    resource: JsonResource::Nodes,
                    limit: 1,
                    remaining: 0,
                    requested: 1,
                })
            )
    ));
}

/// Verifies decoded string payloads enforce the per-string byte maximum.
#[test]
fn test_json_lexical_preflight_checks_decoded_string_bytes() {
    let limits = JsonDecodeLimits::<JsonResource, usize>::builder()
        .value_limits(
            JsonValueLimits::<JsonResource, usize>::builder()
                .string_bytes_limit(ResourceLimit::new(
                    JsonResource::StringBytes,
                    2,
                ))
                .build(),
        )
        .build();
    let mut session = JsonDecodeSession::owned(limits);
    let error = JsonDecoder::new(session)
        .decode_utf8::<IgnoredAny>(br#""\u4e2d""#)
        .expect_err("the decoded three-byte string must exceed the limit");

    assert!(matches!(
        error,
        JsonDecodeError::Budget(error)
            if matches!(
                error.budget_error(),
                Some(BudgetError::LimitExceeded {
                    resource: JsonResource::StringBytes,
                    observed: Observation::Exact(3),
                    maximum: 2,
                })
            )
    ));
}

/// Verifies number limits use the original lexical representation length.
#[test]
fn test_json_lexical_preflight_checks_number_lexical_bytes() {
    let limits = JsonDecodeLimits::<JsonResource, usize>::builder()
        .value_limits(
            JsonValueLimits::<JsonResource, usize>::builder()
                .number_bytes_limit(ResourceLimit::new(
                    JsonResource::NumberBytes,
                    3,
                ))
                .build(),
        )
        .build();
    let mut session = JsonDecodeSession::owned(limits);
    let error = JsonDecoder::new(session)
        .decode_utf8::<IgnoredAny>(b"1e+3")
        .expect_err("all four lexical number bytes must be charged");

    assert!(matches!(
        error,
        JsonDecodeError::Budget(error)
            if matches!(
                error.budget_error(),
                Some(BudgetError::LimitExceeded {
                    resource: JsonResource::NumberBytes,
                    observed: Observation::Exact(4),
                    maximum: 3,
                })
            )
    ));
}

/// Verifies lexical arrays enforce their observed item count.
#[test]
fn test_json_lexical_preflight_checks_sequence_items() {
    let limits = JsonDecodeLimits::<JsonResource, usize>::builder()
        .value_limits(
            JsonValueLimits::<JsonResource, usize>::builder()
                .structure_limits(
                    StructureLimits::builder().sequence_items_limit(
                        ResourceLimit::new(JsonResource::SequenceItems, 1),
                    ),
                )
                .build(),
        )
        .build();
    let mut session = JsonDecodeSession::owned(limits);
    let error = JsonDecoder::new(session)
        .decode_utf8::<IgnoredAny>(b"[null,null]")
        .expect_err("the second array item must exceed the point limit");

    assert!(matches!(
        error,
        JsonDecodeError::Budget(error)
            if matches!(
                error.budget_error(),
                Some(BudgetError::LimitExceeded {
                    resource: JsonResource::SequenceItems,
                    observed: Observation::Exact(2),
                    maximum: 1,
                })
            )
    ));
}

/// Verifies duplicate object keys still count as distinct map entries.
#[test]
fn test_json_lexical_preflight_counts_duplicate_map_entries() {
    let limits = JsonDecodeLimits::<JsonResource, usize>::builder()
        .value_limits(
            JsonValueLimits::<JsonResource, usize>::builder()
                .structure_limits(StructureLimits::builder().map_entries_limit(
                    ResourceLimit::new(JsonResource::MapEntries, 1),
                ))
                .build(),
        )
        .build();
    let mut session = JsonDecodeSession::owned(limits);
    let error = JsonDecoder::new(session)
        .decode_utf8::<IgnoredAny>(br#"{"a":1,"a":2}"#)
        .expect_err("the duplicate second entry must still exceed the limit");

    assert!(matches!(
        error,
        JsonDecodeError::Budget(error)
            if matches!(
                error.budget_error(),
                Some(BudgetError::LimitExceeded {
                    resource: JsonResource::MapEntries,
                    observed: Observation::Exact(2),
                    maximum: 1,
                })
            )
    ));
}

/// Verifies private serde_json token text is an ordinary lexical object key.
#[test]
fn test_json_lexical_preflight_does_not_special_case_private_number_token() {
    const PRIVATE_NUMBER_TOKEN: &str =
        concat!("$", "serde_json", ":", ":private::Number");
    let input = format!(r#"{{"{PRIVATE_NUMBER_TOKEN}":"x"}}"#);
    let limits = JsonDecodeLimits::<JsonResource, usize>::builder()
        .value_limits(
            JsonValueLimits::<JsonResource, usize>::builder()
                .structure_limits(
                    StructureLimits::<JsonResource, usize>::builder()
                        .key_bytes_limit(ResourceLimit::new(
                            JsonResource::KeyBytes,
                            PRIVATE_NUMBER_TOKEN.len() - 1,
                        )),
                )
                .build(),
        )
        .build();
    let mut session = JsonDecodeSession::owned(limits);
    let error = JsonDecoder::new(session)
        .decode_utf8::<IgnoredAny>(input.as_bytes())
        .expect_err("private token text must consume the ordinary key limit");

    assert!(matches!(
        error,
        JsonDecodeError::Budget(error)
            if matches!(
                error.budget_error(),
                Some(BudgetError::LimitExceeded {
                    resource: JsonResource::KeyBytes,
                    observed: Observation::Exact(actual),
                    maximum,
                }) if *actual == PRIVATE_NUMBER_TOKEN.len()
                    && *maximum == PRIVATE_NUMBER_TOKEN.len() - 1
            )
    ));
}

/// Verifies duplicate entries consume key and number payload every time.
#[test]
fn test_json_lexical_preflight_charges_duplicate_entry_payloads() {
    let limits = JsonDecodeLimits::<JsonResource, usize>::builder()
        .value_limits(
            JsonValueLimits::<JsonResource, usize>::builder()
                .payload_bytes_limit(ResourceLimit::new(
                    JsonResource::PayloadBytes,
                    3,
                ))
                .build(),
        )
        .build();
    let mut session = JsonDecodeSession::owned(limits);
    let error = JsonDecoder::new(session)
        .decode_utf8::<IgnoredAny>(br#"{"a":1,"a":2}"#)
        .expect_err("both duplicate key-number pairs must consume payload");

    assert!(matches!(
        error,
        JsonDecodeError::Budget(error)
            if matches!(
                error.budget_error(),
                Some(BudgetError::Insufficient {
                    resource: JsonResource::PayloadBytes,
                    limit: 3,
                    remaining: 0,
                    requested: 1,
                })
            )
    ));
}
