// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Private deserialization helpers for duplicate-key-free JSON values.

mod strict_json_visitor;

pub(super) use strict_json_visitor::StrictJsonVisitor;
