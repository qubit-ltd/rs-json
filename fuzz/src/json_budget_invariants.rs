// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Shared assertions for JSON decode budget transaction invariants.

use qubit_budget::ResourceBudget;
use qubit_budget::json::JsonDecodeSession;
use qubit_budget::json::JsonResource;
use qubit_budget::json::JsonValueBudget;
use qubit_budget::json::JsonValueLimits;
use qubit_json::decode::JsonDecoder;
use qubit_json::value::AccountingJsonValueSeed;
use serde::de::DeserializeSeed;
use serde_json::Value;

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
