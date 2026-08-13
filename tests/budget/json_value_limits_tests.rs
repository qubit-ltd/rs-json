// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests for direction-independent JSON value limits.

use qubit_budget::ResourceLimit;
use qubit_budget::StructureLimits;
use qubit_json::JsonResource;
use qubit_json::JsonValueLimits;

/// Verifies value limits expose every configured point and structural maximum.
#[test]
fn test_json_value_limits_expose_all_configured_values() {
    let structure = StructureLimits::<JsonResource, usize>::empty()
        .with_depth_limit(ResourceLimit::new(JsonResource::Depth, 1))
        .with_nodes_limit(ResourceLimit::new(JsonResource::Nodes, 2))
        .with_sequence_items_limit(ResourceLimit::new(JsonResource::SequenceItems, 3))
        .with_map_entries_limit(ResourceLimit::new(JsonResource::MapEntries, 4))
        .with_key_bytes_limit(ResourceLimit::new(JsonResource::KeyBytes, 5));
    let limits = JsonValueLimits::empty()
        .with_structure_limits(structure)
        .with_string_bytes_limit(ResourceLimit::new(JsonResource::StringBytes, 6))
        .with_number_bytes_limit(ResourceLimit::new(JsonResource::NumberBytes, 7))
        .with_payload_bytes_limit(ResourceLimit::new(JsonResource::PayloadBytes, 8));

    assert_eq!(limits.max_depth(), Some(1));
    assert_eq!(limits.max_nodes(), Some(2));
    assert_eq!(limits.max_sequence_items(), Some(3));
    assert_eq!(limits.max_map_entries(), Some(4));
    assert_eq!(limits.max_key_bytes(), Some(5));
    assert_eq!(limits.max_string_bytes(), Some(6));
    assert_eq!(limits.max_number_bytes(), Some(7));
    assert_eq!(limits.max_payload_bytes(), Some(8));
    assert_eq!(limits.structure_limits(), structure);
}

/// Verifies custom resource identities remain attached to value limits.
#[test]
fn test_json_value_limits_support_custom_resources() {
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum Resource {
        String,
        Number,
        Payload,
    }

    let limits = JsonValueLimits::<Resource, u8>::default()
        .with_string_bytes_limit(ResourceLimit::new(Resource::String, 3))
        .with_number_bytes_limit(ResourceLimit::new(Resource::Number, 4))
        .with_payload_bytes_limit(ResourceLimit::new(Resource::Payload, 5));

    assert_eq!(
        JsonValueLimits::<Resource, u8>::default(),
        JsonValueLimits::default()
    );
    assert_eq!(
        limits.string_bytes_limit().unwrap().resource(),
        &Resource::String
    );
    assert_eq!(
        limits.number_bytes_limit().unwrap().resource(),
        &Resource::Number
    );
    assert_eq!(
        limits.payload_bytes_limit().unwrap().resource(),
        &Resource::Payload
    );
}
