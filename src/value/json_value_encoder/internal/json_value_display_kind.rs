// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0 (the "License");
//    you may not use this file except in compliance with the License.
// =============================================================================
//! Measurement kind used while collecting a value encoder Display projection.

pub(in crate::value::json_value_encoder) enum JsonValueDisplayKind {
    String { depth: usize },
    Key,
}
