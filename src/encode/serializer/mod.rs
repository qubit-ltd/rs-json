// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Budget-aware Serde serializer decorators.

mod budgeted_display_collector;
mod budgeted_key;
mod budgeted_private_value;
mod budgeted_value;
mod display_budget_kind;
mod internal;
mod json_encode_compound;
pub(super) mod json_encode_context;
pub(super) mod json_encode_serializer;
