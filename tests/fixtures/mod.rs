// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Shared typed fixtures for public decoder tests.

mod internal;

pub(crate) use internal::MAX_FUZZ_INPUT_BYTES;
pub(crate) use internal::is_fuzz_input_within_limit;
