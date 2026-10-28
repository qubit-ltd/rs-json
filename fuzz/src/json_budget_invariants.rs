// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Shared assertions for JSON decode budget transaction invariants.

use qubit_budget::ResourceBudget;
use qubit_budget::json::JsonDecodeLimits;
use qubit_budget::json::JsonDecodeSession;
use qubit_budget::json::JsonResource;
use qubit_budget::json::JsonValueBudget;
use qubit_budget::json::JsonValueLimits;
use qubit_json::decode::JsonDecodeErrorKind;
use qubit_json::decode::JsonDecoder;
use qubit_json::value::AccountingJsonValueSeed;
use serde::de::DeserializeSeed;
use serde::de::IgnoredAny;
use serde_json::Value;

use crate::input_limit::bounded_input;
use crate::json_reference::parse_literal_value;

/// Number of control bytes before a budget fuzz payload.
pub const CONTROL_BYTES: usize = 18;

/// Resource indices in the reference usage vector: input, depth, nodes, items,
/// entries, key bytes, string bytes, number bytes, and payload bytes.
const INPUT: usize = 0;
const DEPTH: usize = 1;
const NODES: usize = 2;
const ITEMS: usize = 3;
const ENTRIES: usize = 4;
const KEY_BYTES: usize = 5;
const STRING_BYTES: usize = 6;
const NUMBER_BYTES: usize = 7;
const PAYLOAD_BYTES: usize = 8;
const DIMENSION_COUNT: usize = 9;

/// Splits a complete bounded fuzz input into its fixed control header and
/// payload.
#[must_use]
pub fn split_budget_input(data: &[u8]) -> Option<(&[u8; CONTROL_BYTES], &[u8])> {
    let input = bounded_input(data)?;
    let (control, payload) = input.split_at_checked(CONTROL_BYTES)?;
    Some((control.try_into().ok()?, payload))
}

/// Selects a deterministic budget around a measured baseline from a u16
/// control.
#[must_use]
pub fn select_budget_limit(raw: u16, baseline: usize) -> usize {
    match raw % 5 {
        0 => 0,
        1 => baseline.saturating_sub(1),
        2 => baseline,
        3 => baseline.saturating_add(1),
        _ => (usize::from(raw) / 5) % 4097,
    }
}

/// Checks transaction invariants on the payload and independent resource
/// oracles on a canonical re-encoding of payloads accepted by the literal
/// reference parser.
pub fn assert_json_budget_fuzz_input(data: &[u8]) {
    let Some((control, input)) = split_budget_input(data) else {
        return;
    };
    let limits = JsonValueLimits::<JsonResource, usize>::builder()
        .max_depth(control_limit(control, DEPTH, 64))
        .max_nodes(control_limit(control, NODES, 256))
        .max_sequence_items(control_limit(control, ITEMS, 64))
        .max_map_entries(control_limit(control, ENTRIES, 64))
        .max_key_bytes(control_limit(control, KEY_BYTES, 256))
        .max_string_bytes(control_limit(control, STRING_BYTES, 256))
        .max_number_bytes(control_limit(control, NUMBER_BYTES, 128))
        .max_payload_bytes(control_limit(control, PAYLOAD_BYTES, 512))
        .build();
    assert_json_budget_invariants(input, control_limit(control, INPUT, 4096), limits);

    let Ok(value) = parse_literal_value(input) else {
        return;
    };
    let canonical = serde_json::to_vec(&value).expect("JSON Value serialization succeeds");
    if canonical.len() > 4096 {
        return;
    }
    let usage = reference_usage(&value, canonical.len());
    assert_single_decode(&canonical, usage, usage);

    let dimension = usize::from(control[0]) % DIMENSION_COUNT;
    let start = dimension * 2;
    let raw = u16::from_le_bytes([control[start], control[start + 1]]);
    let mut selected = usage;
    selected[dimension] = select_budget_limit(raw, usage[dimension]);
    assert_single_decode(&canonical, usage, selected);
}

/// Returns the committed node and payload byte counters.
#[must_use = "the accounting snapshot must be checked"]
fn usage(budget: &JsonValueBudget<JsonResource, usize>) -> (Option<usize>, Option<usize>) {
    (budget.used_nodes(), budget.used_payload_bytes())
}

/// Checks that configured counters stay within their limits and balance.
fn assert_balanced(budget: &JsonValueBudget<JsonResource, usize>) {
    for (used, remaining, maximum) in [
        (
            budget.used_nodes(),
            budget.remaining_nodes(),
            budget.limits().max_nodes(),
        ),
        (
            budget.used_payload_bytes(),
            budget.remaining_payload_bytes(),
            budget.limits().max_payload_bytes(),
        ),
    ] {
        match (used, remaining, maximum) {
            (Some(used), Some(remaining), Some(maximum)) => {
                assert!(used <= maximum);
                assert_eq!(used.checked_add(remaining), Some(maximum));
            }
            (None, None, None) => {}
            other => panic!("inconsistent configured accounting: {other:?}"),
        }
    }
}

/// Checks decode accounting, rollback, repeated use, and seed transactions.
///
/// `input` is decoded twice with the supplied value limits and an input-byte
/// budget of `input_bytes`. Invalid input and budget rejection are expected
/// outcomes. This function panics if counters become unbalanced, a failed
/// decode publishes charges, or an uncommitted seed transaction changes the
/// committed counters.
pub fn assert_json_budget_invariants(
    input: &[u8],
    input_bytes: usize,
    value_limits: JsonValueLimits<JsonResource, usize>,
) {
    let mut input_budget = ResourceBudget::new(JsonResource::InputBytes, input_bytes);
    let mut value_budget = JsonValueBudget::new(value_limits);
    {
        let session = JsonDecodeSession::borrowing_input(&mut input_budget, &mut value_budget);
        let mut decoder = JsonDecoder::new(session);
        for _ in 0..2 {
            let before = usage(decoder.session().value_budget());
            let result = decoder.decode_utf8::<Value>(input);
            let after = usage(decoder.session().value_budget());
            if result.is_err() {
                assert_eq!(after, before, "failed decoding must not publish value charges");
            } else {
                assert!(after.0 >= before.0);
                assert!(after.1 >= before.1);
            }
            assert_balanced(decoder.session().value_budget());
        }
        assert_eq!(decoder.session().max_input_bytes(), Some(input_bytes));
    }
    assert!(input_budget.used() <= input_bytes);
    assert_eq!(
        input_budget.used().checked_add(input_budget.remaining()),
        Some(input_bytes)
    );

    for commit_success in [false, true] {
        let before = usage(&value_budget);
        let expected = {
            let mut transaction = value_budget.transaction();
            let mut deserializer = serde_json::Deserializer::from_slice(input);
            let decoded = AccountingJsonValueSeed::new(&mut transaction).deserialize(&mut deserializer);
            if decoded.is_ok() && deserializer.end().is_ok() && commit_success {
                let staged = (transaction.used_nodes(), transaction.used_payload_bytes());
                transaction.commit().expect("fully admitted seed transaction commits");
                staged
            } else {
                before
            }
        };
        assert_eq!(
            usage(&value_budget),
            expected,
            "only complete committed seeds publish charges"
        );
        assert_balanced(&value_budget);
    }
}

fn control_limit(control: &[u8; CONTROL_BYTES], dimension: usize, baseline: usize) -> usize {
    let start = dimension * 2;
    let raw = u16::from_le_bytes([control[start], control[start + 1]]);
    select_budget_limit(raw, baseline)
}

fn reference_usage(value: &Value, input_bytes: usize) -> [usize; DIMENSION_COUNT] {
    let mut measured = [0; DIMENSION_COUNT];
    measured[INPUT] = input_bytes;
    let mut pending = vec![(value, 1_usize)];
    while let Some((value, depth)) = pending.pop() {
        measured[DEPTH] = measured[DEPTH].max(depth);
        measured[NODES] += 1;
        match value {
            Value::Null | Value::Bool(_) => {}
            Value::Number(number) => {
                let bytes = number.to_string().len();
                measured[NUMBER_BYTES] = measured[NUMBER_BYTES].max(bytes);
                measured[PAYLOAD_BYTES] += bytes;
            }
            Value::String(string) => {
                let bytes = string.len();
                measured[STRING_BYTES] = measured[STRING_BYTES].max(bytes);
                measured[PAYLOAD_BYTES] += bytes;
            }
            Value::Array(items) => {
                measured[ITEMS] = measured[ITEMS].max(items.len());
                pending.extend(items.iter().map(|item| (item, depth + 1)));
            }
            Value::Object(entries) => {
                measured[ENTRIES] = measured[ENTRIES].max(entries.len());
                for (key, item) in entries {
                    let bytes = key.len();
                    measured[KEY_BYTES] = measured[KEY_BYTES].max(bytes);
                    measured[PAYLOAD_BYTES] += bytes;
                    pending.push((item, depth + 1));
                }
            }
        }
    }
    measured
}

fn limits_from(measured: [usize; DIMENSION_COUNT]) -> JsonDecodeLimits<JsonResource, usize> {
    JsonDecodeLimits::<JsonResource, usize>::builder()
        .max_input_bytes(measured[INPUT])
        .max_depth(measured[DEPTH])
        .max_nodes(measured[NODES])
        .max_sequence_items(measured[ITEMS])
        .max_map_entries(measured[ENTRIES])
        .max_key_bytes(measured[KEY_BYTES])
        .max_string_bytes(measured[STRING_BYTES])
        .max_number_bytes(measured[NUMBER_BYTES])
        .max_payload_bytes(measured[PAYLOAD_BYTES])
        .build()
}

fn assert_single_decode(input: &[u8], exact: [usize; DIMENSION_COUNT], limits: [usize; DIMENSION_COUNT]) {
    let fits = exact.iter().zip(limits).all(|(used, limit)| *used <= limit);
    let mut decoder = JsonDecoder::with_limits(limits_from(limits));
    let result = decoder.decode_utf8::<IgnoredAny>(input);
    assert_eq!(result.is_ok(), fits, "exact usage {exact:?}, limits {limits:?}");
    if fits {
        assert_eq!(decoder.session().input_budget().unwrap().used(), exact[INPUT]);
        assert_eq!(decoder.session().value_budget().used_nodes(), Some(exact[NODES]));
        assert_eq!(
            decoder.session().value_budget().used_payload_bytes(),
            Some(exact[PAYLOAD_BYTES])
        );
        return;
    }
    let error = result.expect_err("a limit below exact reference usage must reject");
    assert_eq!(error.kind(), JsonDecodeErrorKind::Budget);
    let error = error
        .budget_error()
        .and_then(|error| error.budget_error())
        .expect("budget error has a resource");
    let dimension = exact
        .iter()
        .zip(limits)
        .position(|(used, limit)| *used > limit)
        .unwrap();
    assert_eq!(error.resource(), resource_for(dimension));
    if dimension == INPUT {
        assert_eq!(decoder.session().input_budget().unwrap().used(), 0);
    } else {
        assert_eq!(decoder.session().input_budget().unwrap().used(), exact[INPUT]);
    }
    assert_eq!(decoder.session().value_budget().used_nodes(), Some(0));
    assert_eq!(decoder.session().value_budget().used_payload_bytes(), Some(0));
}

fn resource_for(dimension: usize) -> &'static JsonResource {
    match dimension {
        INPUT => &JsonResource::InputBytes,
        DEPTH => &JsonResource::Depth,
        NODES => &JsonResource::Nodes,
        ITEMS => &JsonResource::SequenceItems,
        ENTRIES => &JsonResource::MapEntries,
        KEY_BYTES => &JsonResource::KeyBytes,
        STRING_BYTES => &JsonResource::StringBytes,
        NUMBER_BYTES => &JsonResource::NumberBytes,
        PAYLOAD_BYTES => &JsonResource::PayloadBytes,
        _ => unreachable!("dimension index is in the reference vector"),
    }
}

#[cfg(test)]
mod tests {
    use super::reference_usage;
    use crate::json_reference::parse_literal_value;

    #[test]
    fn test_reference_measurement_matches_hand_counted_nested_value() {
        let value = parse_literal_value(br#"["x",{"k":2}]"#).expect("sample parses");
        assert_eq!(reference_usage(&value, 13), [13, 3, 4, 2, 1, 1, 1, 1, 3]);
    }

    #[test]
    fn test_reference_measurement_counts_unicode_and_decoded_escapes() {
        let unicode = parse_literal_value(r#"["é"]"#.as_bytes()).expect("Unicode sample parses");
        assert_eq!(reference_usage(&unicode, 6), [6, 2, 2, 1, 0, 0, 2, 0, 2]);

        let escaped = parse_literal_value(br#"["\n"]"#).expect("escaped string parses");
        assert_eq!(reference_usage(&escaped, 6), [6, 2, 2, 1, 0, 0, 1, 0, 1]);
    }

    #[test]
    fn test_reference_measurement_handles_empty_keys_large_and_finite_numbers() {
        let empty_key = parse_literal_value(br#"{"":0}"#).expect("empty key parses");
        assert_eq!(reference_usage(&empty_key, 6), [6, 2, 2, 0, 1, 0, 0, 1, 1]);

        let large = parse_literal_value(b"18446744073709551615").expect("u64 max parses");
        assert_eq!(reference_usage(&large, 20), [20, 1, 1, 0, 0, 0, 0, 20, 20]);

        let float = parse_literal_value(b"1.5").expect("finite float parses");
        assert_eq!(reference_usage(&float, 3), [3, 1, 1, 0, 0, 0, 0, 3, 3]);

        let null = parse_literal_value(b"null").expect("null parses");
        assert_eq!(reference_usage(&null, 4), [4, 1, 1, 0, 0, 0, 0, 0, 0]);
    }
}
