// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests map-key admission through the public JSON value seed.

use qubit_budget::ResourceLimit;
use qubit_budget::StructureLimits;
use qubit_budget::json::JsonResource;
use qubit_budget::json::JsonValueLimits;
use qubit_json::value::JsonValueSeed;
use serde::de::DeserializeSeed;
use serde_json::Deserializer;

/// Verifies a second object key is rejected before it is materialized.
#[test]
fn test_json_key_seed_checks_prospective_object_entry() {
    let limits = JsonValueLimits::empty().with_structure_limits(
        StructureLimits::empty().with_map_entries_limit(ResourceLimit::new(
            JsonResource::MapEntries,
            1,
        )),
    );
    let mut budget = limits.budget();
    let mut transaction = budget.transaction();
    let mut deserializer = Deserializer::from_slice(br#"{"a":null,"b":null}"#);

    assert!(
        JsonValueSeed::new(&mut transaction)
            .deserialize(&mut deserializer)
            .is_err()
    );
}
