// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Regression tests for serde_json's private serialization protocol.

use super::private_struct_kind_tests::PrivateStructKind;

/// Supplies the production module path expected by the included compatibility
/// implementation without loading its source a second time.
mod private_struct_kind {
    pub(super) use super::PrivateStructKind;
}

#[path = "../../../src/budget/internal/serde_json_compat.rs"]
mod serde_json_compat;

use serde_json_compat::classify_private_struct;

/// Private struct name emitted by serde_json arbitrary-precision numbers.
const JSON_NUMBER_TOKEN: &str = concat!("$", "serde_json", ":", ":private::Number");

/// Private struct name emitted by serde_json raw values.
const JSON_RAW_VALUE_TOKEN: &str = concat!("$", "serde_json", ":", ":private::RawValue");

/// Ensures the pinned serde_json private names map to their expected kinds.
#[test]
fn test_classify_private_struct_recognizes_pinned_tokens() {
    assert_eq!(
        classify_private_struct(JSON_NUMBER_TOKEN),
        Some(PrivateStructKind::Number),
    );
    assert_eq!(
        classify_private_struct(JSON_RAW_VALUE_TOKEN),
        Some(PrivateStructKind::RawValue),
    );
}

/// Ensures near-miss private names remain ordinary Serde structs.
#[test]
fn test_classify_private_struct_rejects_forged_names() {
    assert_eq!(
        classify_private_struct(concat!("$", "serde_json", "::private::Number::forged")),
        None,
    );
    assert_eq!(
        classify_private_struct(concat!("serde_json", "::private::RawValue")),
        None,
    );
}
