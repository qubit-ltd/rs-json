// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines callbacks for a read-only JSON tree traversal.

use serde_json::Value;

use super::JsonTreeContext;

/// Receives enter and leave events for budget-admitted JSON tree nodes.
pub trait JsonTreeVisitor {
    /// Domain-specific failure returned by this visitor.
    type Error;

    /// Handles a node after its budget admission and before its descendants.
    fn enter(
        &mut self,
        value: &Value,
        context: JsonTreeContext<'_>,
    ) -> Result<(), Self::Error>;

    /// Handles a node after all of its descendants have been handled.
    fn leave(
        &mut self,
        _value: &Value,
        _context: JsonTreeContext<'_>,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
}
