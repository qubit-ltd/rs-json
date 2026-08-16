// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use qubit_json::value::traverse::JsonTreeContext;
use qubit_json::value::traverse::JsonTreeLocation;
use qubit_json::value::traverse::JsonTreeVisitor;
use serde_json::Value;

struct Visitor;

impl JsonTreeVisitor for Visitor {
    type Error = ();

    fn enter(
        &mut self,
        _value: &Value,
        _context: JsonTreeContext<'_>,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
}

/// Verifies that the default leave callback succeeds.
#[test]
fn test_visitor_default_leave_succeeds() {
    let mut visitor = Visitor;
    let value = Value::Null;
    let context = JsonTreeContext {
        depth: 1,
        location: JsonTreeLocation::Root,
    };

    visitor
        .enter(&value, context)
        .expect("enter should succeed");
    visitor
        .leave(&value, context)
        .expect("default leave should succeed");
}
