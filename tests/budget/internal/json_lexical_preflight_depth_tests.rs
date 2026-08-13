// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests for lexical JSON preflight depth accounting.

use qubit_budget::BudgetError;
use qubit_budget::Observation;
use qubit_budget::ResourceLimit;
use qubit_budget::StructureLimits;
use qubit_budget::json::JsonDecodeLimits;
use qubit_budget::json::JsonDecodeSession;
use qubit_budget::json::JsonResource;
use qubit_budget::json::JsonValueLimits;
use qubit_json::text::JsonDecodeError;
use qubit_json::text::decode_slice;
use serde_json::Value;

/// Verifies that nested values use root-inclusive lexical depth.
#[test]
fn test_json_lexical_preflight_charges_nested_depth() {
    let limits = JsonDecodeLimits::empty().with_value_limits(
        JsonValueLimits::empty().with_structure_limits(
            StructureLimits::empty()
                .with_depth_limit(ResourceLimit::new(JsonResource::Depth, 1)),
        ),
    );
    let mut session = JsonDecodeSession::owned(limits);
    let error = decode_slice::<Value, _, _>(b"[null]", &mut session)
        .expect_err("the nested value should exceed the depth budget");

    assert!(matches!(
        error,
        JsonDecodeError::Budget(error)
            if matches!(
                error.budget_error(),
                Some(BudgetError::LimitExceeded {
                    resource: JsonResource::Depth,
                    observed: Observation::Exact(2),
                    maximum: 1,
                })
            )
    ));
}
