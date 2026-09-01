// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use qubit_json::value::traverse::JsonTreeControl;

/// Verifies the two traversal decisions are distinguishable.
#[test]
fn test_control_variants_are_distinct() {
    assert_ne!(JsonTreeControl::Descend, JsonTreeControl::SkipSubtree);
}
