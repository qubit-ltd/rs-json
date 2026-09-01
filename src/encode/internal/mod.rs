// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Crate-private representation details for JSON encoding errors.

mod json_encode_failure;

pub(in crate::encode) use json_encode_failure::JsonEncodeFailure;
