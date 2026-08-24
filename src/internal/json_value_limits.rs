// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Inspects whether a JSON value traversal has configured limits.

use qubit_budget::ResourceQuantity;
use qubit_budget::json::JsonValueLimits;

/// Reports whether any JSON value resource dimension is configured.
#[must_use]
pub(crate) const fn has_json_value_limits<R, Q>(limits: &JsonValueLimits<R, Q>) -> bool
where
    Q: ResourceQuantity,
{
    limits.max_depth().is_some()
        || limits.max_nodes().is_some()
        || limits.max_sequence_items().is_some()
        || limits.max_map_entries().is_some()
        || limits.max_key_bytes().is_some()
        || limits.max_string_bytes().is_some()
        || limits.max_number_bytes().is_some()
        || limits.max_payload_bytes().is_some()
}
