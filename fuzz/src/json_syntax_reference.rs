// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Independent JSON syntax validation for fuzz and test support.
//!
//! This reference checks complete JSON syntax without materializing numbers or
//! object values. It is intentionally not a complete formatter-contract oracle:
//! `serde_json::value::RawValue` may accept Unicode cases such as isolated
//! surrogate escapes differently from the formatter. Its parser also has a
//! recursion limit, so callers must keep the existing shallow reference-domain
//! boundary and leave deeper inputs to their dedicated invariants.

use serde_json::value::RawValue;

/// Checks complete JSON syntax without materializing numbers or objects.
pub fn validate_json_syntax(input: &[u8]) -> Result<(), serde_json::Error> {
    let _: &RawValue = serde_json::from_slice(input)?;
    Ok(())
}
