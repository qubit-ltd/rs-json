// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Budget accounting for already materialized JSON values.
// qubit-style: allow source-test-pair
// qubit-style: allow explicit-imports

use std::fmt::Debug;
use std::io::Error as IoError;

use qubit_budget::ResourceQuantity;
use serde_json::Error as JsonError;
use serde_json::Value;

use super::JsonSerdeError;
use super::JsonValueBudget;

/// Charges every resource represented by one materialized JSON value.
///
/// This iterative traversal uses the same resource semantics as lexical JSON
/// admission: containers and scalars each charge one node, object keys share
/// the payload budget, and string and number byte limits use their UTF-8 JSON
/// representation lengths.
pub fn account_value<R, Q>(
    value: &Value,
    budget: &mut JsonValueBudget<R, Q>,
) -> Result<(), JsonSerdeError<R, Q>>
where
    R: Clone,
    Q: ResourceQuantity,
{
    let mut pending = vec![(value, 1_usize)];
    while let Some((value, depth)) = pending.pop() {
        budget
            .enter_node_usize(depth)
            .map_err(JsonSerdeError::from)?;
        match value {
            Value::Null | Value::Bool(_) => {}
            Value::Number(number) => budget
                .consume_number_bytes_usize(number.as_str().len())
                .map_err(JsonSerdeError::from)?,
            Value::String(text) => budget
                .consume_string_bytes_usize(text.len())
                .map_err(JsonSerdeError::from)?,
            Value::Array(values) => {
                let items = values.len();
                budget
                    .check_sequence_items_usize(items)
                    .map_err(JsonSerdeError::from)?;
                let child_depth =
                    depth.checked_add(1).ok_or_else(invalid_json)?;
                for value in values.iter().rev() {
                    pending.push((value, child_depth));
                }
            }
            Value::Object(entries) => {
                let count = entries.len();
                budget
                    .check_map_entries_usize(count)
                    .map_err(JsonSerdeError::from)?;
                let child_depth =
                    depth.checked_add(1).ok_or_else(invalid_json)?;
                for (key, value) in entries.iter().rev() {
                    budget
                        .consume_key_bytes_usize(key.len())
                        .map_err(JsonSerdeError::from)?;
                    pending.push((value, child_depth));
                }
            }
        }
    }
    Ok(())
}

fn invalid_json<R, Q>() -> JsonSerdeError<R, Q>
where
    Q: Copy + Debug,
{
    JsonSerdeError::Json(JsonError::io(IoError::other(
        "JSON traversal depth overflow",
    )))
}
