// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Budget-aware Serde serializer decorators.

mod internal;

pub(in crate::encode) use internal::JsonEncodeContext;
pub(in crate::encode) use internal::JsonEncodeSerializer;
