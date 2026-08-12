// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests for JSON resource identities.

use qubit_json::JsonResource;

/// Verifies JSON resource identities retain basic value semantics.
#[test]
fn test_json_resource_is_clone_copy_and_equatable() {
    /// Checks the trait bounds expected by JSON resource identities.
    fn assert_clone_copy_and_equatable<T: Clone + Copy + PartialEq + Eq>() {}

    assert_clone_copy_and_equatable::<JsonResource>();
    assert_ne!(JsonResource::InputBytes, JsonResource::Depth);
    assert_ne!(JsonResource::Nodes, JsonResource::SequenceItems);
    assert_ne!(JsonResource::MapEntries, JsonResource::StringBytes);
    assert_ne!(JsonResource::NumberBytes, JsonResource::InputBytes);
    assert_ne!(JsonResource::PayloadBytes, JsonResource::NumberBytes);
}
