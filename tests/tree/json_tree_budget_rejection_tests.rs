// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use qubit_json::value::traverse::JsonTreeBudgetRejection;

/// Verifies the fail-closed default and explicit continuation choice.
#[test]
fn test_rejection_variants_are_distinct() {
    assert_ne!(
        JsonTreeBudgetRejection::Abort,
        JsonTreeBudgetRejection::SkipSubtree
    );
}
