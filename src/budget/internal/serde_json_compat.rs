// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Compatibility boundary for serde_json's private serialization protocol.

use super::private_struct_kind::PrivateStructKind;

/// Private struct name emitted for arbitrary-precision JSON numbers.
const JSON_NUMBER_TOKEN: &str =
    concat!("$", "serde_json", ":", ":private::Number");

/// Private struct name emitted for raw JSON fragments.
const JSON_RAW_VALUE_TOKEN: &str =
    concat!("$", "serde_json", ":", ":private::RawValue");

/// Classifies one serde_json private struct name.
///
/// Returns the corresponding private shape for a pinned serde_json token, or
/// `None` when `name` is an ordinary Serde struct name.
pub(crate) fn classify_private_struct(
    name: &'static str,
) -> Option<PrivateStructKind> {
    match name {
        JSON_NUMBER_TOKEN => Some(PrivateStructKind::Number),
        JSON_RAW_VALUE_TOKEN => Some(PrivateStructKind::RawValue),
        _ => None,
    }
}
