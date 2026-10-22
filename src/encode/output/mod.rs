// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Budget-aware JSON output sinks.

mod internal;

pub(super) use internal::JsonOutputAccounting;
pub(super) use internal::JsonOutputBuffer;
pub(super) use internal::JsonOutputWriter;
