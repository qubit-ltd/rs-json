// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use qubit_json::value::traverse::JsonTreeLocation;

/// Verifies that locations retain array indexes and object keys.
#[test]
fn test_location_variants_retain_parent_coordinates() {
    assert_eq!(
        JsonTreeLocation::ArrayElement { index: 2 },
        JsonTreeLocation::ArrayElement { index: 2 },
    );
    assert_eq!(
        JsonTreeLocation::ObjectValue { key: "name" },
        JsonTreeLocation::ObjectValue { key: "name" },
    );
}
