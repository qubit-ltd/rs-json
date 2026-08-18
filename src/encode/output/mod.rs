// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Budget-aware JSON output sinks.

pub(super) mod json_output_accounting;
pub(super) mod json_output_buffer;
pub(super) mod json_output_writer;

pub(super) use json_output_accounting::JsonOutputAccounting;
pub(super) use json_output_buffer::JsonOutputBuffer;
pub(super) use json_output_writer::JsonOutputWriter;
