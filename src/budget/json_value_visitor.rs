// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
// =============================================================================
//! Explicit-stack traversal of materialized JSON values.
// qubit-style: allow source-test-pair

use qubit_budget::JsonValueBudget;
use qubit_budget::ResourceQuantity;
use serde_json::Error as JsonError;
use serde_json::Value;

use super::JsonSerdeError;

/// Receives callbacks for every node and object key in a JSON tree.
pub trait JsonValueVisitor<R, Q>
where
    Q: ResourceQuantity,
{
    /// Visits one node after its budget admission and before its children.
    fn visit_value(
        &mut self,
        value: &Value,
        depth: usize,
    ) -> Result<(), JsonSerdeError<R, Q>>;

    /// Visits one object key before its associated value.
    fn visit_key(&mut self, _key: &str) -> Result<(), JsonSerdeError<R, Q>> {
        Ok(())
    }
}

/// Walks a materialized JSON tree without recursion while enforcing a value
/// budget.
pub fn walk_json_value<R, Q, V>(
    value: &Value,
    budget: &mut JsonValueBudget<R, Q>,
    visitor: &mut V,
) -> Result<(), JsonSerdeError<R, Q>>
where
    R: Clone,
    Q: ResourceQuantity,
    V: JsonValueVisitor<R, Q>,
{
    let mut pending = vec![(value, 1_usize)];
    while let Some((value, depth)) = pending.pop() {
        budget
            .enter_node_usize(depth)
            .map_err(JsonSerdeError::from)?;
        visitor.visit_value(value, depth)?;
        match value {
            Value::Null | Value::Bool(_) => {}
            Value::Number(number) => budget
                .consume_number_bytes_usize(number.as_str().len())
                .map_err(JsonSerdeError::from)?,
            Value::String(text) => budget
                .consume_string_bytes_usize(text.len())
                .map_err(JsonSerdeError::from)?,
            Value::Array(values) => {
                budget
                    .check_sequence_items_usize(values.len())
                    .map_err(JsonSerdeError::from)?;
                let child_depth = depth.checked_add(1).ok_or_else(|| {
                    JsonSerdeError::Json(JsonError::io(std::io::Error::other(
                        "JSON traversal depth overflow",
                    )))
                })?;
                pending.extend(
                    values.iter().rev().map(|child| (child, child_depth)),
                );
            }
            Value::Object(entries) => {
                budget
                    .check_map_entries_usize(entries.len())
                    .map_err(JsonSerdeError::from)?;
                let child_depth = depth.checked_add(1).ok_or_else(|| {
                    JsonSerdeError::Json(JsonError::io(std::io::Error::other(
                        "JSON traversal depth overflow",
                    )))
                })?;
                for (key, child) in entries.iter().rev() {
                    budget
                        .consume_key_bytes_usize(key.len())
                        .map_err(JsonSerdeError::from)?;
                    visitor.visit_key(key)?;
                    pending.push((child, child_depth));
                }
            }
        }
    }
    Ok(())
}
