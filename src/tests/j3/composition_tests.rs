// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Cross-API composition and compatibility contract tests.

use qubit_budget::json::JsonResource;
use qubit_budget::json::JsonValueBudget;
use qubit_budget::json::JsonValueLimits;

use crate::lexical::JsonLexicalScanError;
use crate::lexical::JsonLexicalScanner;
use crate::lexical::JsonNumberModel;
use crate::value::internal::JsonValueMaterializationError;
use crate::value::internal::LexicalJsonValueBuilder;

#[test]
fn test_builder_consumer_stops_at_first_materialization_failure() {
    let mut budget = JsonValueBudget::new(JsonValueLimits::<JsonResource, usize>::new());
    let mut transaction = budget.transaction();
    let mut builder = LexicalJsonValueBuilder::new(JsonNumberModel::Integer64);
    let result =
        JsonLexicalScanner::new(&mut transaction, false).scan_with_consumer(br#"{"x":0,"x":1,"tail":]"#, &mut builder);

    assert!(matches!(
        result,
        Err(JsonLexicalScanError::Consumer(
            JsonValueMaterializationError::DuplicateKey { .. }
        ))
    ));
}
