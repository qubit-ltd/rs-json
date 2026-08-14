// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Private implementation types for shared decoder test fixtures.

mod byte_buffer_visitor;
mod fuzz_input_limit;
mod fuzz_limit;
mod fuzz_limit_tests;

pub(crate) use byte_buffer_visitor::ByteBufferVisitor;
pub(crate) use fuzz_input_limit::MAX_FUZZ_INPUT_BYTES;
pub(crate) use fuzz_input_limit::is_fuzz_input_within_limit;
