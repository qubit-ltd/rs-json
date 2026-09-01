// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Public RawValue protocol compatibility for strict encoding.

use super::private_struct_kind::PrivateStructKind;

/// Recognizes serde_json's RawValue struct protocol.
pub(in crate::encode) enum SerdeJsonCompat {}

impl SerdeJsonCompat {
    /// Private struct name emitted for raw JSON fragments.
    const RAW_VALUE_TOKEN: &'static str = concat!("$", "serde_json", ":", ":private::RawValue");

    /// Classifies one serde_json RawValue struct name.
    ///
    /// Returns the RawValue shape for serde_json's token, or `None` when
    /// `name` is an ordinary Serde struct name.
    pub(in crate::encode) fn classify_private_struct(name: &'static str) -> Option<PrivateStructKind> {
        (name == Self::RAW_VALUE_TOKEN).then_some(PrivateStructKind::RawValue)
    }
}
