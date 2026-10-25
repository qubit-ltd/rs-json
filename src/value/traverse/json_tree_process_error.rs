// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Exposes the error type produced while processing JSON trees.

#[path = "json_tree_process_error/json_tree_process_error.rs"]
mod error;

pub use error::JsonTreeProcessError;
