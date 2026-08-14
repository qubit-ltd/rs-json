// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests for serde_json private struct shape classification values.

#[path = "../../../src/budget/internal/private_struct_kind.rs"]
mod private_struct_kind;

pub(super) use private_struct_kind::PrivateStructKind;

/// Ensures each private struct kind remains distinct and debuggable.
#[test]
fn test_private_struct_kind_variants_are_distinct() {
    assert_ne!(PrivateStructKind::Number, PrivateStructKind::RawValue);
    assert_eq!(format!("{:?}", PrivateStructKind::Number), "Number");
    assert_eq!(format!("{:?}", PrivateStructKind::RawValue), "RawValue");
}
