// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Private output accounting and sink implementations.

mod json_output_accounting;
mod json_output_buffer;
mod json_output_writer;

pub(in crate::encode) use json_output_accounting::JsonOutputAccounting;
pub(in crate::encode) use json_output_buffer::JsonOutputBuffer;
pub(in crate::encode) use json_output_writer::JsonOutputWriter;
