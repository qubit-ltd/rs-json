// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Private serde_json protocol compatibility for strict encoding.

use super::private_struct_kind::PrivateStructKind;

/// Recognizes serde_json's pinned private struct protocol.
pub(in crate::encode) enum SerdeJsonCompat {}

impl SerdeJsonCompat {
    /// Private struct name emitted for arbitrary-precision JSON numbers.
    const NUMBER_TOKEN: &'static str =
        concat!("$", "serde_json", ":", ":private::Number");

    /// Private struct name emitted for raw JSON fragments.
    const RAW_VALUE_TOKEN: &'static str =
        concat!("$", "serde_json", ":", ":private::RawValue");

    /// Classifies one serde_json private struct name.
    ///
    /// Returns the corresponding private shape for a pinned serde_json token,
    /// or `None` when `name` is an ordinary Serde struct name.
    pub(in crate::encode) fn classify_private_struct(
        name: &'static str,
    ) -> Option<PrivateStructKind> {
        match name {
            Self::NUMBER_TOKEN => Some(PrivateStructKind::Number),
            Self::RAW_VALUE_TOKEN => Some(PrivateStructKind::RawValue),
            _ => None,
        }
    }
}
