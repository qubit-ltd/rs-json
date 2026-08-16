// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Private serde_json struct shapes recognized by the budget serializer.

/// Private serde_json struct shapes recognized by the budget serializer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::encode) enum PrivateStructKind {
    /// An arbitrary-precision JSON number.
    Number,

    /// A raw JSON fragment.
    RawValue,
}
