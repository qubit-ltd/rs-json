// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
#[derive(Clone, Copy)]
pub(in crate::format) enum ContainerKind {
    /// An array whose children are separated and indented as values.
    Array,
    /// An object whose keys and values alternate.
    Object,
}
