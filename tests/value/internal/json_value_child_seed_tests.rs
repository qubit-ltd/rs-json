// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests nested child admission through the public JSON value seed.

use qubit_budget::ResourceLimit;
use qubit_budget::StructureLimits;
use qubit_budget::json::JsonResource;
use qubit_budget::json::JsonValueLimits;
use qubit_json::value::JsonValueSeed;
use serde::de::DeserializeSeed;
use serde_json::Deserializer;

/// Verifies an excess array child is rejected before materialization.
#[test]
fn test_json_value_child_seed_checks_prospective_array_item() {
    let limits = JsonValueLimits::empty().with_structure_limits(
        StructureLimits::new()
            .with_sequence_items_limit(ResourceLimit::new(JsonResource::SequenceItems, 1)),
    );
    let mut budget = limits.budget();
    let mut transaction = budget.transaction();
    let mut deserializer = Deserializer::from_slice(br#"[null,null]"#);

    assert!(
        JsonValueSeed::new(&mut transaction)
            .deserialize(&mut deserializer)
            .is_err()
    );
}
