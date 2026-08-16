// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use qubit_json::value::traverse::JsonTreeContext;
use qubit_json::value::traverse::JsonTreeLocation;

/// Verifies that context preserves the root-inclusive depth and location.
#[test]
fn test_context_preserves_depth_and_location() {
    let context = JsonTreeContext {
        depth: 1,
        location: JsonTreeLocation::Root,
    };

    assert_eq!(context.depth, 1);
    assert_eq!(context.location, JsonTreeLocation::Root);
}
