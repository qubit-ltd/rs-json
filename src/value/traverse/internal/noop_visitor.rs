// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Defines the no-op visitor used for accounting-only traversal.

use serde_json::Value;

use super::super::JsonTreeContext;
use super::super::JsonTreeVisitor;

/// Accepts every admitted node without adding domain behavior.
pub(in crate::value::traverse) struct NoopVisitor;

impl JsonTreeVisitor for NoopVisitor {
    type Error = std::convert::Infallible;

    /// Accepts an entered node.
    #[inline(always)]
    fn enter(&mut self, _value: &Value, _context: JsonTreeContext<'_>) -> Result<(), Self::Error> {
        Ok(())
    }
}
