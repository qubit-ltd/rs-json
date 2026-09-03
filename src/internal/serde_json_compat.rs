// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Central compatibility constants for serde_json's public serialization
//! behavior that is represented by private protocol tokens.

/// Struct and field token emitted by `serde_json::value::RawValue`.
///
/// This is intentionally the only production definition of the private token.
/// Upgrade audits for `serde_json` must verify the RawValue protocol here.
pub(crate) const SERDE_JSON_RAW_VALUE_TOKEN: &str = concat!("$", "serde_json", ":", ":private::RawValue");
