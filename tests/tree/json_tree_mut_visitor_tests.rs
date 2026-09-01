// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use qubit_json::value::traverse::JsonTreeContext;
use qubit_json::value::traverse::JsonTreeControl;
use qubit_json::value::traverse::JsonTreeLocation;
use qubit_json::value::traverse::JsonTreeMutVisitor;
use serde_json::Value;

struct Visitor;

impl JsonTreeMutVisitor for Visitor {
    type Error = ();

    fn visit(
        &mut self,
        _value: &mut Value,
        _context: JsonTreeContext<'_>,
    ) -> Result<JsonTreeControl, Self::Error> {
        Ok(JsonTreeControl::SkipSubtree)
    }
}

/// Verifies that mutable visitors can select subtree skipping.
#[test]
fn test_mut_visitor_selects_skip_subtree() {
    let mut visitor = Visitor;
    let mut value = Value::Null;
    let control = visitor
        .visit(
            &mut value,
            JsonTreeContext {
                depth: 1,
                location: JsonTreeLocation::Root,
            },
        )
        .expect("fixture visitor should succeed");

    assert_eq!(control, JsonTreeControl::SkipSubtree);
}
