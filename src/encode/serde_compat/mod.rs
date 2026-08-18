// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Compatibility adapters for serde_json's private JSON shapes.

pub(super) mod private_struct_kind;
pub(super) mod serde_json_compat;

pub(super) use private_struct_kind::PrivateStructKind;
pub(super) use serde_json_compat::SerdeJsonCompat;
